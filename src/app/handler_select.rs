use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::discovery::{extract_branch_context, find_session_jsonl};
use crate::types::*;
use crate::util::now_secs;

impl super::App {
    pub(super) fn handle_settings_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.view.status.clear();
            }
            KeyCode::Char('a') => {
                // Add workspace — go to new workspace flow
                self.view.input_mode = InputMode::NewWorkspaceName;
                self.input_buffer.clear();
                self.new_workspace_name = None;
                self.view.status = "Workspace name (Esc = cancel):".into();
            }
            KeyCode::Char('d') => {
                // Delete last workspace (simple approach: delete the one currently selected in sidebar)
                if self.sessions.workspaces.len() > 1 {
                    let count = self.sessions.workspaces.len();
                    self.view.status = format!("Delete workspace #{count}? y/n");
                    self.pending_delete = Some(TreeNode::Workspace(count - 1));
                    self.pending_batch_delete = false;
                    self.view.input_mode = InputMode::ConfirmDelete;
                } else {
                    self.view.status = "Cannot delete the only workspace.".into();
                }
            }
            KeyCode::Char('r') => {
                // Rename last workspace
                if !self.sessions.workspaces.is_empty() {
                    let wi = self.sessions.workspaces.len() - 1;
                    self.view.input_mode = InputMode::RenameWorkspace;
                    self.rename_workspace_target = Some(wi);
                    self.input_buffer = self.sessions.workspaces[wi].name.clone();
                    self.view.status = "Edit workspace name (Enter=confirm, Esc=cancel):".into();
                }
            }
            KeyCode::Char('k') => {
                self.view.input_mode = InputMode::KeybindView;
                self.view.status = "Keybindings (any key to close)".into();
            }
            KeyCode::Char('t') => {
                self.open_theme_panel();
            }
            KeyCode::Char('b') => {
                // Toggle token budget: set a default daily 100k token budget if none set,
                // or clear it
                if self.token_budget.is_some() {
                    self.token_budget = None;
                    self.popup.budget_alert = None;
                    self.view.status = "Token budget cleared.".into();
                } else {
                    self.token_budget = Some(crate::budget::TokenBudget {
                        daily_tokens: Some(100_000),
                        weekly_tokens: None,
                        daily_cost: None,
                        weekly_cost: None,
                    });
                    self.view.status =
                        "Token budget set: 100k daily tokens. Edit config.json to customize."
                            .into();
                }
                self.save_config();
            }
            _ => {}
        }
        Action::Continue
    }

    pub(super) fn handle_theme_select_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.theme_list.clear();
                self.view.picker_query.clear();
                self.view.status.clear();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let len = self.theme_list.len();
                if len > 0 {
                    let cur = self.theme_list_state.selected().unwrap_or(0);
                    self.theme_list_state
                        .select(Some(if cur == 0 { len - 1 } else { cur - 1 }));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.theme_list.len();
                if len > 0 {
                    let cur = self.theme_list_state.selected().unwrap_or(0);
                    self.theme_list_state.select(Some((cur + 1) % len));
                }
            }
            KeyCode::Enter => {
                if let Some(idx) = self.theme_list_state.selected()
                    && let Some(name) = self.theme_list.get(idx).cloned()
                {
                    self.view.theme_name = name;
                    self.view.theme = self.view.theme_name.theme();
                    self.view.status = format!("Theme: {}", self.view.theme_name.label());
                    self.save_config();
                }
                self.view.input_mode = InputMode::None;
                self.theme_list.clear();
                self.view.picker_query.clear();
            }
            KeyCode::Backspace => {
                self.view.picker_query.pop();
            }
            KeyCode::Char(c) => {
                self.view.picker_query.push(c);
            }
            _ => {}
        }
        Action::Continue
    }

    pub(super) fn handle_template_select_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.view.picker_query.clear();
                self.view.status.clear();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let len = self.templates.len();
                if len > 0 {
                    let cur = self.template_state.selected().unwrap_or(0);
                    self.template_state
                        .select(Some(if cur == 0 { len - 1 } else { cur - 1 }));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.templates.len();
                if len > 0 {
                    let cur = self.template_state.selected().unwrap_or(0);
                    self.template_state.select(Some((cur + 1) % len));
                }
            }
            KeyCode::Enter => {
                if let Some(idx) = self.template_state.selected()
                    && idx < self.templates.len()
                {
                    let tmpl = self.templates[idx].clone();
                    let ws_idx = tmpl
                        .workspace_id
                        .as_ref()
                        .and_then(|id| self.sessions.workspaces.iter().position(|ws| ws.id == *id))
                        .or_else(|| {
                            self.selected_node().and_then(|n| match n {
                                TreeNode::Workspace(wi) | TreeNode::Session(wi, _) => Some(*wi),
                                _ => None,
                            })
                        });
                    if let Some(wi) = ws_idx {
                        let ws_path = self.workspace_cwd(wi);
                        self.spawn_with_agent(tmpl.agent, None)?;
                        if let Some(ref prompt) = tmpl.initial_prompt
                            && let Some(pi) = self.ptys.active_pty
                            && let Some(slot) = self.ptys.ptys.get(pi)
                        {
                            let expanded = crate::template::expand_template_vars(prompt, &ws_path);
                            let data = format!("{expanded}\n");
                            if let Err(e) = slot.handle.write_input(data.as_bytes()) {
                                self.view.status = format!("Write error: {e}");
                            }
                        }
                        self.view.status = format!("Launched template: {}", tmpl.name);
                    } else {
                        self.view.status = "Template workspace not found.".into();
                    }
                }
                self.view.input_mode = InputMode::None;
                self.view.picker_query.clear();
            }
            KeyCode::Backspace => {
                self.view.picker_query.pop();
            }
            KeyCode::Char(c) => {
                self.view.picker_query.push(c);
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    pub(super) fn handle_chain_select_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.view.status.clear();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let len = self.chains.chains.len();
                if len > 0 {
                    let cur = self.chains.chain_state.selected().unwrap_or(0);
                    self.chains
                        .chain_state
                        .select(Some(if cur == 0 { len - 1 } else { cur - 1 }));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.chains.chains.len();
                if len > 0 {
                    let cur = self.chains.chain_state.selected().unwrap_or(0);
                    self.chains.chain_state.select(Some((cur + 1) % len));
                }
            }
            KeyCode::Enter => {
                if let Some(idx) = self.chains.chain_state.selected()
                    && idx < self.chains.chains.len()
                {
                    let chain = self.chains.chains[idx].clone();
                    if chain.steps.is_empty() {
                        self.view.status = "Chain has no steps.".into();
                        self.view.input_mode = InputMode::None;
                        return Action::Continue;
                    }

                    // Find workspace path from current selection
                    let ws_path = self
                        .selected_node()
                        .and_then(|n| self.node_workspace_path(n));
                    let workspace_path = match ws_path {
                        Some(p) => p,
                        None => {
                            self.view.status = "Select a workspace or session first.".into();
                            self.view.input_mode = InputMode::None;
                            return Action::Continue;
                        }
                    };

                    // Set up active chain state
                    let total_steps = chain.steps.len();
                    let chain_name = chain.name.clone();

                    self.chains.active_chain = Some(crate::chain::ActiveChain {
                        chain_name: chain_name.clone(),
                        current_step: 0,
                        total_steps,
                        workspace_path: workspace_path.clone(),
                        prev_output: None,
                    });

                    // Start the first step
                    let first_step = chain.steps[0].clone();
                    let agent = first_step.agent;
                    let prompt = first_step.prompt;

                    // Find workspace index
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
                        let name = Some(format!("{chain_name}-step1"));
                        let env = self.project_env(&workspace_path);
                        let pty = crate::pty::PtyHandle::spawn(
                            agent,
                            &workspace_path,
                            None,
                            name.as_deref(),
                            chat_size,
                            &env,
                            &[],
                        );

                        match pty {
                            Ok(pty_handle) => {
                                let pty_id = self.next_pty_id();
                                let pty_idx = self.ptys.ptys.len();
                                let pt = crate::discovery::ProjectType::detect(&workspace_path);
                                self.ptys.ptys.push(PtySlot {
                                    id: pty_id.clone(),
                                    handle: pty_handle,
                                    info: RunningInfo {
                                        workspace_path,
                                        title: format!("{chain_name} [1/{total_steps}]"),
                                        session_id: None,
                                        started_at: now_secs(),
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
                                self.register_pty(&pty_id, &self.ptys.ptys[pty_idx]);
                                self.ptys.active_pty = Some(pty_idx);
                                self.view.focus = Focus::Chat;
                                self.rebuild_tree();

                                // Inject prompt for step 1 with delay
                                if !prompt.is_empty() {
                                    let fire_at_ms = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis()
                                        .try_into()
                                        .unwrap_or(u64::MAX)
                                        + 1500;
                                    let pending = PendingInput {
                                        fire_at_ms,
                                        text: prompt,
                                    };
                                    self.ptys.pending_inputs.push(pending);
                                }

                                self.view.status = format!(
                                    "Chain '{}': Step 1/{} — {}",
                                    chain_name,
                                    total_steps,
                                    agent.label()
                                );
                            }
                            Err(e) => {
                                self.view.status = format!("Chain start failed: {e}");
                                self.chains.active_chain = None;
                            }
                        }
                    } else {
                        self.view.status = "Workspace not found for chain.".into();
                        self.chains.active_chain = None;
                    }
                }
                self.view.input_mode = InputMode::None;
            }
            _ => {}
        }
        Action::Continue
    }
    pub(super) fn handle_automation_select_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.view.picker_query.clear();
                self.view.status.clear();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let len = self.automations.len();
                if len > 0 {
                    let cur = self.automation_state.selected().unwrap_or(0);
                    self.automation_state
                        .select(Some(if cur == 0 { len - 1 } else { cur - 1 }));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.automations.len();
                if len > 0 {
                    let cur = self.automation_state.selected().unwrap_or(0);
                    self.automation_state.select(Some((cur + 1) % len));
                }
            }
            KeyCode::Enter => {
                if let Some(idx) = self.automation_state.selected()
                    && idx < self.automations.len()
                {
                    let auto = &self.automations[idx];
                    if let Some(pi) = self.ptys.active_pty {
                        let ws_path = self
                            .ptys
                            .ptys
                            .get(pi)
                            .map(|s| s.info.workspace_path.clone());
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis()
                            .try_into()
                            .unwrap_or(u64::MAX);
                        let mut offset_ms = 0u64;
                        for step in &auto.steps {
                            offset_ms += step.delay_ms;
                            let text = ws_path
                                .as_ref()
                                .map(|p| crate::template::expand_template_vars(&step.text, p))
                                .unwrap_or_else(|| step.text.clone());
                            self.ptys.pending_inputs.push(PendingInput {
                                fire_at_ms: now_ms + offset_ms,
                                text,
                            });
                        }
                        self.view.status = format!(
                            "Queued automation: {} ({} steps)",
                            auto.name,
                            auto.steps.len()
                        );
                    } else {
                        self.view.status = "No active PTY. Open a session first.".into();
                    }
                }
                self.view.input_mode = InputMode::None;
                self.view.picker_query.clear();
            }
            KeyCode::Backspace => {
                self.view.picker_query.pop();
            }
            KeyCode::Char(c) => {
                self.view.picker_query.push(c);
            }
            _ => {}
        }
        Action::Continue
    }
    pub(super) fn handle_browse_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.new_workspace_name = None;
                self.view.status = "Cancelled.".into();
            }
            KeyCode::Enter => {
                self.browse_select();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.browse_move(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.browse_move(-1);
            }
            KeyCode::Backspace | KeyCode::Char('h') => {
                self.browse_up();
            }
            _ => {}
        }
        Action::Continue
    }

    pub(super) fn handle_branch_select_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.popup.branch_points.clear();
                self.view.status.clear();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let len = self.popup.branch_points.len();
                if len > 0 {
                    let cur = self.branch_state.selected().unwrap_or(0);
                    self.branch_state
                        .select(Some(if cur == 0 { len - 1 } else { cur - 1 }));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.popup.branch_points.len();
                if len > 0 {
                    let cur = self.branch_state.selected().unwrap_or(0);
                    self.branch_state.select(Some((cur + 1) % len));
                }
            }
            KeyCode::Enter => {
                if let Some(idx) = self.branch_state.selected()
                    && idx < self.popup.branch_points.len()
                {
                    let branch_index = self.popup.branch_points[idx].index;
                    let node = self.selected_node();
                    let session = match node {
                        Some(TreeNode::Session(_wi, si)) => {
                            self.sessions.sessions.get(*si).cloned()
                        }
                        _ => None,
                    };
                    if let Some(session) = session {
                        let jsonl_path = find_session_jsonl(&session);
                        if let Some(ref jsonl_path) = jsonl_path {
                            if let Some(ctx) = extract_branch_context(jsonl_path, branch_index) {
                                self.spawn_with_agent(session.agent, None)?;
                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis()
                                    .try_into()
                                    .unwrap_or(u64::MAX);
                                self.ptys.pending_inputs.push(PendingInput {
                                    fire_at_ms: now_ms + 3000,
                                    text: ctx,
                                });
                                self.view.status = format!(
                                    "Branched from turn {} — context queued",
                                    branch_index + 1
                                );
                            } else {
                                self.view.status = "Failed to extract branch context.".into();
                            }
                        } else {
                            self.view.status = "Cannot find session JSONL.".into();
                        }
                    } else {
                        self.view.status = "Session no longer available.".into();
                    }
                }
                self.view.input_mode = InputMode::None;
                self.popup.branch_points.clear();
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    pub(super) fn handle_plugin_list_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.view.status.clear();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let len = self.plugins.len();
                if len > 0 {
                    let cur = self.plugin_state.selected().unwrap_or(0);
                    self.plugin_state
                        .select(Some(if cur == 0 { len - 1 } else { cur - 1 }));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.plugins.len();
                if len > 0 {
                    let cur = self.plugin_state.selected().unwrap_or(0);
                    self.plugin_state.select(Some((cur + 1) % len));
                }
            }
            KeyCode::Enter => {
                if let Some(idx) = self.plugin_state.selected()
                    && idx < self.plugins.len()
                {
                    let plugin = self.plugins[idx].clone();
                    // Build command with workspace/session context
                    let ws_path = self
                        .selected_node()
                        .and_then(|n| match n {
                            TreeNode::Workspace(wi) | TreeNode::Session(wi, _) => {
                                self.sessions.workspaces[*wi].path.clone()
                            }
                            _ => None,
                        })
                        .unwrap_or_default();
                    let ws_str = ws_path.to_string_lossy().to_string();
                    let cmd = plugin
                        .command
                        .replace("{workspace}", &ws_str)
                        .replace("{session_id}", "current");

                    let output = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .output();

                    match output {
                        Ok(out) => {
                            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                            let combined: String = stdout
                                .lines()
                                .chain(stderr.lines())
                                .map(|l| format!("{l}\n"))
                                .collect();
                            self.plugin_output = vec![format!("$ {}", plugin.name)];
                            for line in stdout.lines().chain(stderr.lines()) {
                                self.plugin_output.push(line.to_string());
                            }
                            self.plugin_scroll = 0;
                            self.view.input_mode = InputMode::PluginOutput;
                            self.view.status = format!("Plugin '{}' completed", plugin.name);
                            // Parse and handle JSON actions
                            self.handle_plugin_actions(&combined, &plugin.name);
                        }
                        Err(e) => {
                            self.view.status = format!("Plugin '{}' failed: {}", plugin.name, e);
                            self.view.input_mode = InputMode::None;
                        }
                    }
                }
            }
            _ => {}
        }
        Action::Continue
    }

    pub(super) fn handle_agent_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.pending_session_name = None;
                self.view.status = "Cancelled.".into();
            }
            KeyCode::Enter => {
                self.confirm_input()?;
            }
            KeyCode::Char(ch) => {
                let lowered = ch.to_ascii_lowercase();
                if lowered == 'j' {
                    let len = self.available_agents.len();
                    if len > 0 {
                        let cur = self.agent_state.selected().unwrap_or(0).min(len - 1);
                        let next = (cur + 1) % len;
                        self.agent_state.select(Some(next));
                    }
                } else if lowered == 'k' {
                    let len = self.available_agents.len();
                    if len > 0 {
                        let cur = self.agent_state.selected().unwrap_or(0).min(len - 1);
                        let prev = (cur + len - 1) % len;
                        self.agent_state.select(Some(prev));
                    }
                } else if let Some(&agent) = Agent::ALL.iter().find(|a| a.shortcut_key() == lowered) {
                    if self.available_agents.contains(&agent) {
                        self.agent_state.select(Some(
                            self.available_agents
                                .iter()
                                .position(|a| *a == agent)
                                .unwrap_or(0),
                        ));
                        self.confirm_input()?;
                    }
                }
            }
            KeyCode::Down => {
                let len = self.available_agents.len();
                if len > 0 {
                    let cur = self.agent_state.selected().unwrap_or(0).min(len - 1);
                    let next = (cur + 1) % len;
                    self.agent_state.select(Some(next));
                }
            }
            KeyCode::Up => {
                let len = self.available_agents.len();
                if len > 0 {
                    let cur = self.agent_state.selected().unwrap_or(0).min(len - 1);
                    let prev = (cur + len - 1) % len;
                    self.agent_state.select(Some(prev));
                }
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    pub(super) fn handle_plugin_output_key(&mut self, key: KeyEvent) -> Action {
        let line_count = self.plugin_output.len();
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if self.plugin_scroll + 1 < line_count {
                    self.plugin_scroll += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.plugin_scroll = self.plugin_scroll.saturating_sub(1);
            }
            KeyCode::PageDown => {
                // Scroll by ~10 lines
                self.plugin_scroll = (self.plugin_scroll + 10).min(line_count.saturating_sub(1));
            }
            KeyCode::PageUp => {
                self.plugin_scroll = self.plugin_scroll.saturating_sub(10);
            }
            KeyCode::Home => {
                self.plugin_scroll = 0;
            }
            KeyCode::End => {
                self.plugin_scroll = line_count.saturating_sub(1);
            }
            _ => {
                self.view.input_mode = InputMode::None;
                self.plugin_output.clear();
                self.plugin_scroll = 0;
            }
        }
        Action::Continue
    }

    /// Parse plugin output for JSON actions and handle them.
    fn handle_plugin_actions(&mut self, output: &str, plugin_name: &str) {
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(action) = serde_json::from_str::<PluginAction>(trimmed) {
                match action {
                    PluginAction::CreateSession { agent, prompt: _ } => {
                        let agent = agent
                            .as_deref()
                            .and_then(Agent::from_label)
                            .or_else(|| self.available_agents.first().copied());
                        if let Some(agent) = agent
                            && let Err(e) = self.spawn_with_agent(agent, None)
                        {
                            self.view.status =
                                format!("Plugin {plugin_name} create_session failed: {e}");
                        }
                    }
                    PluginAction::SwitchWorkspace { id } => {
                        if let Some(id) = id {
                            let target = self.sessions.workspaces.iter().position(|w| {
                                w.path
                                    .as_ref()
                                    .map(|p| p.to_string_lossy().ends_with(&id))
                                    .unwrap_or(false)
                                    || w.name == id
                            });
                            if let Some(wi) = target {
                                let tree_idx =
                                    self.sessions.tree.iter().position(
                                        |n| matches!(n, TreeNode::Workspace(i) if *i == wi),
                                    );
                                if let Some(idx) = tree_idx {
                                    self.sessions.tree_state.select(Some(idx));
                                }
                            }
                        }
                    }
                    PluginAction::Notify { message } => {
                        self.send_desktop_notification(&format!("Plugin: {plugin_name}"), &message);
                        self.view.status = message;
                    }
                }
            }
            // Non-JSON lines are just displayed as text (already in plugin_output)
        }
    }

    pub(super) fn handle_conflict_resolve_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('i' | 'I') => {
                self.isolate_conflicts();
            }
            KeyCode::Esc | KeyCode::Char('d' | 'D') => {
                self.view.input_mode = InputMode::None;
                self.popup.conflict_warnings.clear();
                self.view.status = "Conflict warning dismissed.".into();
            }
            _ => {}
        }
        Action::Continue
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::theme::ThemeName;
    use crate::types::InputMode;

    fn theme_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn fresh_app() -> super::super::App {
        let mut app = super::super::tests::test_app(vec![], vec![]);
        app.view.input_mode = InputMode::ThemeSelect;
        app.theme_list = vec![ThemeName::Dark, ThemeName::Light];
        app.theme_list_state.select(Some(0));
        app
    }

    // 1. open_theme_panel clears picker_query
    #[test]
    fn open_theme_panel_clears_picker_query() {
        let mut app = fresh_app();
        app.view.picker_query = "stale".into();
        app.open_theme_panel();
        assert!(app.view.picker_query.is_empty());
        assert_eq!(app.view.input_mode, InputMode::ThemeSelect);
    }

    // 2. Typing chars appends to picker_query
    #[test]
    fn typing_chars_appends_to_picker_query() {
        let mut app = fresh_app();
        app.handle_theme_select_key(theme_key(KeyCode::Char('d')));
        app.handle_theme_select_key(theme_key(KeyCode::Char('a')));
        app.handle_theme_select_key(theme_key(KeyCode::Char('r')));
        assert_eq!(app.view.picker_query, "dar");
        // Mode should remain ThemeSelect
        assert_eq!(app.view.input_mode, InputMode::ThemeSelect);
    }

    // 3. Backspace removes from picker_query
    #[test]
    fn backspace_removes_from_picker_query() {
        let mut app = fresh_app();
        app.view.picker_query = "mo".into();
        app.handle_theme_select_key(theme_key(KeyCode::Backspace));
        assert_eq!(app.view.picker_query, "m");
        // Backspace on empty query is a no-op
        app.view.picker_query.clear();
        app.handle_theme_select_key(theme_key(KeyCode::Backspace));
        assert!(app.view.picker_query.is_empty());
    }

    // 4. Escape clears query and exits picker mode
    #[test]
    fn escape_clears_and_exits() {
        let mut app = fresh_app();
        app.view.picker_query = "dark".into();
        app.handle_theme_select_key(theme_key(KeyCode::Esc));
        assert!(app.view.picker_query.is_empty());
        assert_eq!(app.view.input_mode, InputMode::None);
        assert!(app.theme_list.is_empty());
        assert!(app.view.status.is_empty());
    }

    // 5. Enter applies theme and clears picker_query
    #[test]
    fn enter_applies_theme_and_clears_query() {
        let mut app = fresh_app();
        app.view.picker_query = "da".into();
        // List has [Dark, Light], selection on index 0 (Dark)
        app.handle_theme_select_key(theme_key(KeyCode::Enter));
        assert_eq!(app.view.theme_name, ThemeName::Dark);
        assert!(app.view.picker_query.is_empty());
        assert_eq!(app.view.input_mode, InputMode::None);
        assert!(app.theme_list.is_empty());
    }

    // 6. Up/Down navigation wraps and preserves picker_query
    #[test]
    fn up_down_navigates_and_preserves_query() {
        let mut app = fresh_app();
        app.view.picker_query = "li".into();
        // Down: 0 -> 1
        app.handle_theme_select_key(theme_key(KeyCode::Down));
        assert_eq!(app.theme_list_state.selected(), Some(1));
        assert_eq!(app.view.picker_query, "li");
        // Down again wraps: 1 -> 0
        app.handle_theme_select_key(theme_key(KeyCode::Down));
        assert_eq!(app.theme_list_state.selected(), Some(0));
        // Up wraps back: 0 -> 1
        app.handle_theme_select_key(theme_key(KeyCode::Up));
        assert_eq!(app.theme_list_state.selected(), Some(1));
        assert_eq!(app.view.input_mode, InputMode::ThemeSelect);
    }

    // 7. Enter on second item applies correct theme
    #[test]
    fn enter_on_second_item_applies_light() {
        let mut app = fresh_app();
        // Navigate to Light (index 1)
        app.handle_theme_select_key(theme_key(KeyCode::Down));
        app.handle_theme_select_key(theme_key(KeyCode::Enter));
        assert_eq!(app.view.theme_name, ThemeName::Light);
        assert_eq!(app.view.input_mode, InputMode::None);
    }
}
