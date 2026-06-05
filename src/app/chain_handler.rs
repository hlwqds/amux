use super::*;

impl App {
    /// Execute a deferred chain step (called after the PTY iter_mut loop).
    pub(super) fn execute_chain_step(
        &mut self,
        chain_step: Option<crate::chain::ActiveChain>,
        chain_completed: bool,
    ) {
        if chain_completed {
            if let Some(ref chain) = self.chains.active_chain {
                self.view.status = format!(
                    "Chain '{}' complete ({} steps)",
                    chain.chain_name, chain.total_steps
                );
            }
            self.chains.active_chain = None;
            return;
        }
        let Some(updated) = chain_step else { return };

        // Clone chain data to avoid borrow conflicts with &mut self methods below.
        let chain_data = self
            .chains
            .chains
            .iter()
            .find(|c| c.name == updated.chain_name)
            .cloned();

        let Some(chain_config) = chain_data else {
            self.chains.active_chain = None;
            return;
        };

        // Validate the output of the just-completed step against its schema
        if updated.current_step > 0 {
            let completed_idx = updated.current_step - 1;
            if let Some(completed_step) = chain_config.steps.get(completed_idx)
                && let Some(ref schema) = completed_step.expected_output_schema
                && let Some(ref output) = updated.prev_output
            {
                Self::validate_step_output(&chain_config.name, completed_idx, output, schema);
            }
        }

        // Find workspace index for spawn
        let wi = self
            .sessions
            .workspaces
            .iter()
            .position(|ws| ws.path.as_deref() == Some(updated.workspace_path.as_path()));

        let Some(wi) = wi else {
            self.chains.active_chain = None;
            return;
        };

        match chain_config.mode {
            crate::chain::ChainMode::Sequential => {
                self.spawn_sequential_step(&chain_config, updated, wi);
            }
            crate::chain::ChainMode::Parallel => {
                self.spawn_parallel_steps(&chain_config, updated, wi);
            }
        }
    }

    /// Spawn the next sequential step in a chain.
    fn spawn_sequential_step(
        &mut self,
        chain_config: &crate::chain::SessionChain,
        updated: crate::chain::ActiveChain,
        wi: usize,
    ) {
        let next_step = chain_config.steps.get(updated.current_step).cloned();

        let Some(step) = next_step else {
            self.chains.active_chain = None;
            return;
        };

        let workspace_path = updated.workspace_path.clone();
        let prompt = step.prompt.replace(
            "{prev_output}",
            updated.prev_output.as_deref().unwrap_or(""),
        );
        let agent = step.agent;
        let chain_name = updated.chain_name.clone();
        let step_num = updated.current_step + 1;
        let total = updated.total_steps;

        // Update active chain state
        self.chains.active_chain = Some(updated);

        self.spawn_chain_pty(wi, &workspace_path, &chain_name, step_num, total, agent, &prompt);
    }

    /// Spawn all remaining steps simultaneously in parallel mode.
    fn spawn_parallel_steps(
        &mut self,
        chain_config: &crate::chain::SessionChain,
        updated: crate::chain::ActiveChain,
        wi: usize,
    ) {
        let workspace_path = updated.workspace_path.clone();
        let chain_name = updated.chain_name.clone();
        let total = updated.total_steps;
        let start_idx = updated.current_step;

        // In parallel mode, spawn all remaining steps at once.
        let remaining: &[crate::chain::ChainStep] = &chain_config.steps[start_idx..];

        if remaining.is_empty() {
            self.chains.active_chain = None;
            return;
        }

        // Mark the chain as active with the first parallel step.
        self.chains.active_chain = Some(updated);

        let count = remaining.len();
        for (offset, step) in remaining.iter().enumerate() {
            let step_num = start_idx + offset + 1;
            let prompt = step.prompt.replace("{prev_output}", "");
            self.spawn_chain_pty(
                wi,
                &workspace_path,
                &chain_name,
                step_num,
                total,
                step.agent,
                &prompt,
            );
        }

        self.view.status = format!(
            "Chain '{}': {} parallel agents launched ({}/{})",
            chain_name,
            count,
            start_idx + count,
            total,
        );
    }

    /// Shared helper to spawn a single chain PTY and inject its prompt.
    #[allow(clippy::too_many_arguments)]
    fn spawn_chain_pty(
        &mut self,
        wi: usize,
        workspace_path: &std::path::Path,
        chain_name: &str,
        step_num: usize,
        total: usize,
        agent: crate::types::Agent,
        prompt: &str,
    ) {
        let tree_idx = self
            .sessions
            .tree
            .iter()
            .position(|n| matches!(n, TreeNode::Workspace(idx) if *idx == wi));
        if let Some(ti) = tree_idx {
            self.sessions.tree_state.select(Some(ti));
        }
        let chat_size = self.chat_size();
        let name = Some(format!("{}-step{}", chain_name, step_num));
        let env = self.project_env(workspace_path);
        let pty_result = crate::pty::PtyHandle::spawn(
            agent,
            workspace_path,
            None,
            name.as_deref(),
            chat_size,
            &env,
        );
        if let Ok(pty) = pty_result {
            let pty_id = self.next_pty_id();
            let idx = self.ptys.ptys.len();
            let pt = crate::discovery::ProjectType::detect(workspace_path);
            self.ptys.ptys.push(PtySlot {
                id: pty_id.clone(),
                handle: pty,
                info: RunningInfo {
                    workspace_path: workspace_path.to_path_buf(),
                    title: format!("{} [{}/{}]", chain_name, step_num, total),
                    session_id: None,
                    started_at: crate::util::now_secs(),
                    completed: false,
                    agent,
                    git_info: GitInfo::default(),
                    check_status: CheckStatus::Pending,
                    diff_summary: DiffSummary::default(),
                    project_type: pt,
                    worktree_branch: None,
                    snapshot_commit: None,
                },
                last_screen_hash: 0,
                last_recording_at: std::time::Instant::now(),
                process_stats: None,
            });
            self.register_pty(&pty_id, &self.ptys.ptys[idx]);
            self.ptys.active_pty = Some(idx);
            self.view.focus = Focus::Chat;
            self.rebuild_tree();

            // Inject the prompt via delayed write
            if !prompt.is_empty() {
                let fire_at_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64
                    + 1500;
                let pending = PendingInput {
                    fire_at_ms,
                    text: prompt.to_string(),
                };
                self.ptys.pending_inputs.push(pending);
            }

            self.view.status = format!(
                "Chain '{}': Step {}/{} — {}",
                chain_name,
                step_num,
                total,
                agent.label()
            );
        } else {
            self.view.status = format!("Chain step {} failed to spawn", step_num);
            self.chains.active_chain = None;
        }
    }

    /// Validate a completed step's output against its expected JSON Schema.
    ///
    /// Currently performs a lightweight structural check. Full JSON Schema
    /// validation can be added later with a dedicated crate.
    fn validate_step_output(
        chain_name: &str,
        step_idx: usize,
        output: &str,
        schema: &serde_json::Value,
    ) {
        // Attempt to parse output as JSON; if it fails, that's one form of mismatch.
        let parsed: Option<serde_json::Value> = serde_json::from_str(output).ok();
        if let Some(ref value) = parsed {
            if let Some(expected_type) = schema.get("type").and_then(|t| t.as_str()) {
                let actual_matches = match expected_type {
                    "string" => value.is_string(),
                    "number" => value.is_number(),
                    "integer" => value.is_i64() || value.is_u64(),
                    "boolean" => value.is_boolean(),
                    "array" => value.is_array(),
                    "object" => value.is_object(),
                    _ => true, // unknown type constraint, pass
                };
                if !actual_matches {
                    tracing::warn!(
                        chain_name,
                        step_idx,
                        expected_type,
                        "Chain step output schema validation failed: type mismatch"
                    );
                }
            }
        } else {
            // Output is not valid JSON but a schema was expected
            tracing::warn!(
                chain_name,
                step_idx,
                "Chain step output schema validation failed: output is not valid JSON"
            );
        }
    }
}
