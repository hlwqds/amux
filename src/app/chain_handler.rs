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

        // Look up the next chain step configuration
        let next_step = self
            .chains
            .chains
            .iter()
            .find(|c| c.name == updated.chain_name)
            .and_then(|c| c.steps.get(updated.current_step))
            .cloned();

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

        // Find workspace index for spawn
        let wi = self
            .sessions
            .workspaces
            .iter()
            .position(|ws| ws.path.as_deref() == Some(workspace_path.as_path()));

        if let Some(wi) = wi {
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
            let env = self.project_env(&workspace_path);
            let pty_result = crate::pty::PtyHandle::spawn(
                agent,
                &workspace_path,
                None,
                name.as_deref(),
                chat_size,
                &env,
            );
            if let Ok(pty) = pty_result {
                let pty_id = self.next_pty_id();
                let idx = self.ptys.ptys.len();
                let pt = crate::discovery::ProjectType::detect(&workspace_path);
                self.ptys.ptys.push(PtySlot {
                    id: pty_id.clone(),
                    handle: pty,
                    info: RunningInfo {
                        workspace_path: workspace_path.clone(),
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
                        text: prompt,
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
    }
}
