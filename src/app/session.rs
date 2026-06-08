use std::path::Path;

use anyhow::Result;

use crate::config::save_session_title;
use crate::pty::PtyHandle;
use crate::types::*;
use crate::util::*;

impl super::App {
    fn spawn_session(&mut self, agent: Agent) -> Result<()> {
        let name = self.pending_session_name.take();
        self.spawn_with_agent(agent, name)
    }

    /// Spawn a session with the given agent. Runs pre-flight checks first:
    /// if any warnings are found and the project config requests a popup,
    /// stores the result and returns Ok without spawning.
    pub(super) fn spawn_with_agent(&mut self, agent: Agent, name: Option<String>) -> Result<()> {
        // Pre-flight check: if we have pending preflight state, skip the check.
        if self.popup.preflight_result.is_none()
            && let Some(node) = self.selected_node()
            && let Some(path) = self.node_workspace_path(node)
        {
            let pc = self
                .sessions
                .project_configs
                .get(&path)
                .cloned()
                .unwrap_or_default();
            if matches!(pc.preflight.mode, crate::types::PreflightMode::Popup) {
                let result = crate::preflight::run_preflight(&path);
                if result.has_warnings() {
                    self.popup.preflight_result = Some(result);
                    self.popup.preflight_workspace = Some(path);
                    self.popup.preflight_agent = Some(agent);
                    self.popup.preflight_session_name = name;
                    self.view.input_mode = InputMode::PreflightConfirm;
                    self.view.status =
                        "Pre-flight checks — Enter/p=proceed, f=fix, Esc=cancel".into();
                    return Ok(());
                }
            } else if pc.preflight.require_clean_git {
                let result = crate::preflight::run_preflight(&path);
                let dirty = result.checks.iter().any(|(k, s)| {
                    k == "Git status" && matches!(s, crate::preflight::CheckStatus::Warn(_))
                });
                if dirty {
                    self.popup.preflight_result = Some(result);
                    self.popup.preflight_workspace = Some(path);
                    self.popup.preflight_agent = Some(agent);
                    self.popup.preflight_session_name = name;
                    self.view.input_mode = InputMode::PreflightConfirm;
                    self.view.status =
                        "Git working tree dirty — Enter/p=proceed, Esc=cancel".into();
                    return Ok(());
                }
            }
        }
        // Clear any stale preflight state and proceed with actual spawn.
        self.popup.preflight_result = None;
        self.popup.preflight_workspace = None;
        self.popup.preflight_agent = None;
        self.popup.preflight_session_name = None;
        self.spawn_with_agent_inner(agent, &name)
    }

    pub(super) fn spawn_with_agent_inner(
        &mut self,
        agent: Agent,
        name: &Option<String>,
    ) -> Result<()> {
        let chat_size = self.chat_size();
        match self.selected_node().cloned() {
            Some(TreeNode::Workspace(wi)) => {
                let path = self.workspace_cwd(wi);
                let display_name = name.clone().unwrap_or_else(|| "unnamed".into());
                self.view.status = format!(
                    "Starting {} '{}' in {}...",
                    agent.label(),
                    display_name,
                    self.sessions.workspaces[wi].name
                );
                let env = self.project_env(&path);
                let snapshot = Self::capture_snapshot_commit(&path);
                let pty = match PtyHandle::spawn(
                    agent,
                    &path,
                    None,
                    name.as_deref(),
                    chat_size,
                    &env,
                    &[],
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        let msg = if e.to_string().contains("not found")
                            || e.to_string().contains("No such file")
                        {
                            format!("{} not found. {}", agent.label(), agent.install_hint())
                        } else {
                            format!("Failed to spawn {}: {}", agent.label(), e)
                        };
                        self.view.status = msg;
                        anyhow::bail!(e);
                    }
                };
                let pty_id = self.next_pty_id();
                let idx = self.ptys.ptys.len();
                self.ptys.ptys.push(PtySlot {
                    id: pty_id.clone(),
                    handle: pty,
                    info: {
                        let pt = crate::discovery::ProjectType::detect(&path);
                        RunningInfo {
                            workspace_path: path.clone(),
                            title: display_name,
                            session_id: None,
                            started_at: now_secs(),
                            completed: false,
                            agent,
                            git_info: GitInfo::default(),
                            check_status: CheckStatus::Pending,
                            diff_summary: DiffSummary::default(),
                            project_type: pt,
                            worktree_branch: None,
                            snapshot_commit: snapshot,
                        }
                    },
                    last_screen_hash: 0,
                    last_recording_at: std::time::Instant::now(),
                    process_stats: None,
                });
                self.register_pty(&pty_id, &self.ptys.ptys[idx]);
                self.ptys.active_pty = Some(idx);
                self.inject_knowledge(&path);
                self.view.focus = Focus::Chat;
                self.rebuild_tree();
                self.update_related_sessions();
            }
            Some(TreeNode::Session(_wi, si)) => {
                let session = self.sessions.sessions[si].clone();
                if let Some(existing) = self.pty_index_for_session(&session.id) {
                    self.ptys.active_pty = Some(existing);
                    self.view.focus = Focus::Chat;
                    self.update_related_sessions();
                    return Ok(());
                }
                let path = session.workspace_path.clone();
                let id = session.id.clone();
                let title = session.title;
                self.view.status = format!("Resuming: {}...", &id[..8.min(id.len())]);
                let env = self.project_env(&path);
                let snapshot = Self::capture_snapshot_commit(&path);
                let pty = match PtyHandle::spawn(
                    agent,
                    &path,
                    Some(&id),
                    name.as_deref(),
                    chat_size,
                    &env,
                    &[],
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        let msg = if e.to_string().contains("not found")
                            || e.to_string().contains("No such file")
                        {
                            format!("{} not found. {}", agent.label(), agent.install_hint())
                        } else {
                            format!("Failed to resume session: {e}")
                        };
                        self.view.status = msg;
                        anyhow::bail!(e);
                    }
                };
                let pty_id = self.next_pty_id();
                let idx = self.ptys.ptys.len();
                self.ptys.ptys.push(PtySlot {
                    id: pty_id.clone(),
                    handle: pty,
                    info: {
                        let pt = crate::discovery::ProjectType::detect(&path);
                        RunningInfo {
                            workspace_path: path.clone(),
                            title,
                            session_id: Some(id),
                            started_at: now_secs(),
                            completed: false,
                            agent,
                            git_info: GitInfo::default(),
                            check_status: CheckStatus::Pending,
                            diff_summary: DiffSummary::default(),
                            project_type: pt,
                            worktree_branch: None,
                            snapshot_commit: snapshot,
                        }
                    },
                    last_screen_hash: 0,
                    last_recording_at: std::time::Instant::now(),
                    process_stats: None,
                });
                self.register_pty(&pty_id, &self.ptys.ptys[idx]);
                self.ptys.active_pty = Some(idx);
                self.inject_knowledge(&path);
                self.view.focus = Focus::Chat;
                self.rebuild_tree();
                self.update_related_sessions();
            }
            Some(TreeNode::ActiveTab(pi)) => {
                self.ptys.active_pty = Some(pi);
                self.view.focus = Focus::Chat;
                self.update_related_sessions();
            }
            Some(TreeNode::AgentHeader(_))
            | Some(TreeNode::ArchivedHeader)
            | Some(TreeNode::WorkspaceWarning(_, _))
            | Some(TreeNode::ArchivedSession(_, _))
            | Some(TreeNode::PinnedWorkspace)
            | Some(TreeNode::RecentWorkspace)
            | None => {}
        }
        Ok(())
    }

    pub(super) fn confirm_input(&mut self) -> Result<()> {
        match self.view.input_mode {
            InputMode::SessionName => {
                let name = if self.input_buffer.trim().is_empty() {
                    None
                } else {
                    Some(self.input_buffer.clone())
                };
                self.pending_session_name = name;
                self.input_buffer.clear();
                // Check if project config specifies a default agent and no filter is active
                let project_default = self
                    .selected_node()
                    .cloned()
                    .and_then(|n| self.node_workspace_path(&n))
                    .and_then(|path| self.default_agent_for_workspace(&path));
                if let Some(agent) = project_default
                    && self.view.agent_filter.is_none()
                    && self.available_agents.contains(&agent)
                {
                    self.view.input_mode = InputMode::None;
                    self.spawn_session(agent)?;
                    return Ok(());
                }
                if self.available_agents.len() == 1 {
                    let agent = self.available_agents[0];
                    self.view.input_mode = InputMode::None;
                    self.spawn_session(agent)?;
                } else {
                    self.view.input_mode = InputMode::SelectAgent;
                    self.agent_state.select(Some(0));
                    self.view.status =
                        "Select agent \u{00b7} Enter to confirm \u{00b7} Esc to cancel".into();
                }
            }
            InputMode::RenameSession => {
                if let Some(si) = self.rename_target {
                    let new_title = if self.input_buffer.trim().is_empty() {
                        self.sessions.sessions[si].id[..8.min(self.sessions.sessions[si].id.len())]
                            .to_string()
                    } else {
                        self.input_buffer.clone()
                    };
                    let _ = save_session_title(&self.sessions.sessions[si].id, &new_title);
                    self.sessions.sessions[si].title = new_title.clone();
                    self.view.status = format!("Renamed to: {new_title}");
                    self.rebuild_tree();
                }
                self.view.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.rename_target = None;
            }
            InputMode::NewWorkspaceName => {
                let name = self.input_buffer.trim().to_string();
                if name.is_empty() {
                    self.view.status = "Workspace name cannot be empty.".into();
                    self.view.input_mode = InputMode::None;
                    self.input_buffer.clear();
                } else {
                    self.new_workspace_name = Some(name);
                    self.input_buffer.clear();
                    self.start_browse_dir();
                }
            }
            InputMode::RenameWorkspace => {
                if let Some(wi) = self.rename_workspace_target {
                    let new_name = if self.input_buffer.trim().is_empty() {
                        self.sessions.workspaces[wi].name.clone()
                    } else {
                        self.input_buffer.clone()
                    };
                    self.sessions.workspaces[wi].name = new_name.clone();
                    self.save_config();
                    self.view.status = format!("Workspace renamed to: {new_name}");
                    self.rebuild_tree();
                }
                self.view.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.rename_workspace_target = None;
            }
            InputMode::SelectAgent => {
                if let Some(idx) = self.agent_state.selected()
                    && let Some(&agent) = self.available_agents.get(idx)
                {
                    self.view.input_mode = InputMode::None;
                    self.spawn_session(agent)?;
                    return Ok(());
                }
            }
            InputMode::SemanticSearch => {
                let query = self.input_buffer.trim().to_string();
                if query.is_empty() {
                    self.view.input_mode = InputMode::None;
                    self.input_buffer.clear();
                    self.view.status = "Cancelled.".into();
                } else {
                    self.search.results = self.search.index.search(&query, 10);
                    self.input_buffer.clear();
                    if self.search.results.is_empty() {
                        self.view.status = format!("No results for '{query}'.");
                        self.view.input_mode = InputMode::None;
                    } else {
                        self.search.result_state.select(Some(0));
                        self.view.status = format!(
                            "BM25: '{}' ({} hits · j/k · Enter · Esc)",
                            query,
                            self.search.results.len()
                        );
                        // Stay in SemanticSearch mode for result navigation.
                    }
                }
            }
            InputMode::None
            | InputMode::BrowseDir
            | InputMode::Search
            | InputMode::ConfirmDelete
            | InputMode::Help
            | InputMode::SessionPreview
            | InputMode::TagFilter
            | InputMode::Settings
            | InputMode::TemplateSelect
            | InputMode::AutomationSelect
            | InputMode::BranchSelect
            | InputMode::Stats
            | InputMode::TokenStats
            | InputMode::DiffView
            | InputMode::RemoteView
            | InputMode::PluginList
            | InputMode::PluginOutput
            | InputMode::Timeline
            | InputMode::ConflictWarning
            | InputMode::AgentRecommend
            | InputMode::CrossSearch
            | InputMode::KeybindView
            | InputMode::SummaryPreview
            | InputMode::ConflictResolve
            | InputMode::ThemeSelect
            | InputMode::RollbackConfirm
            | InputMode::BudgetWarning
            | InputMode::ChainSelect
            | InputMode::PreflightConfirm
            | InputMode::ScrollbackSearch => {}
        }
        Ok(())
    }

    pub(super) fn start_rename(&mut self) {
        match self.selected_node().cloned() {
            Some(TreeNode::Workspace(wi)) if wi < self.sessions.workspaces.len() => {
                self.view.input_mode = InputMode::RenameWorkspace;
                self.rename_workspace_target = Some(wi);
                self.input_buffer = self.sessions.workspaces[wi].name.clone();
                self.view.status = "Edit workspace name (Enter=confirm, Esc=cancel):".into();
            }
            Some(TreeNode::Session(_, si)) if si < self.sessions.sessions.len() => {
                self.view.input_mode = InputMode::RenameSession;
                self.rename_target = Some(si);
                self.input_buffer = self.sessions.sessions[si].title.clone();
                self.view.status = "Edit session name (Enter=confirm, Esc=cancel):".into();
            }
            _ => {}
        }
    }

    pub(super) fn start_new_workspace(&mut self) {
        self.view.input_mode = InputMode::NewWorkspaceName;
        self.input_buffer.clear();
        self.new_workspace_name = None;
        self.view.status = "Workspace name (Esc = cancel):".into();
    }

    /// Inject workspace knowledge into the active PTY if auto_inject_knowledge is enabled.
    fn inject_knowledge(&mut self, workspace_path: &Path) {
        let auto_inject = self
            .sessions
            .project_configs
            .get(workspace_path)
            .map(|pc| pc.auto_inject_knowledge)
            .unwrap_or(true);
        if !auto_inject {
            return;
        }
        let knowledge = crate::knowledge::load_knowledge(workspace_path);
        let prompt = crate::knowledge::generate_injection_prompt(&knowledge);
        if prompt.is_empty() {
            return;
        }
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        self.ptys.pending_inputs.push(PendingInput {
            fire_at_ms: now_ms + 3000,
            text: prompt,
        });
    }

    /// Toggle pinned state for the selected session.
    pub(super) fn toggle_pin(&mut self) {
        if let Some(TreeNode::Session(_, si)) = self.selected_node().cloned()
            && si < self.sessions.sessions.len()
        {
            let session = &mut self.sessions.sessions[si];
            session.pinned = !session.pinned;
            let pinned = session.pinned;
            let id = session.id.clone();
            if let Err(e) = crate::config::save_session_pinned(&id, pinned) {
                self.view.status = format!("Failed to save pin: {e}");
            } else {
                self.view.status = if pinned {
                    "📌 Pinned".into()
                } else {
                    "Unpinned".into()
                };
            }
            self.rebuild_tree();
        }
    }

    /// Capture the current git HEAD commit hash for rollback snapshots.
    fn capture_snapshot_commit(workspace_path: &Path) -> Option<String> {
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(workspace_path)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::app::tests::{sess, test_app, ws};
    use crate::types::InputMode;

    // ── Test 1: start_rename on a session sets RenameSession mode ──
    #[test]
    fn start_rename_session_sets_mode_and_target() {
        let mut app = test_app(
            vec![ws("w1", "my-workspace", "/tmp/ws1")],
            vec![sess("deadbeef", "my session", "/tmp/ws1")],
        );
        // Tree: [PinnedWorkspace, RecentWorkspace, Workspace(0), WorkspaceWarning(0,..), Session(0,0)]
        // Session is at index 4 because /tmp/ws1 doesn't exist, triggering a warning node.
        app.sessions.tree_state.select(Some(4));
        app.start_rename();
        assert_eq!(app.view.input_mode, InputMode::RenameSession);
        assert_eq!(app.rename_target, Some(0));
        assert_eq!(app.input_buffer, "my session");
        assert!(app.view.status.contains("Edit session name"));
    }

    // ── Test 2: start_rename on a workspace sets RenameWorkspace mode ──
    #[test]
    fn start_rename_workspace_sets_mode_and_target() {
        let mut app = test_app(vec![ws("w1", "original-name", "/tmp/ws1")], vec![]);
        // Tree has: [PinnedWorkspace, RecentWorkspace, Workspace(0), WorkspaceWarning(0,..)]. Select the workspace node at index 2.
        app.sessions.tree_state.select(Some(2));
        app.start_rename();
        assert_eq!(app.view.input_mode, InputMode::RenameWorkspace);
        assert_eq!(app.rename_workspace_target, Some(0));
        assert_eq!(app.input_buffer, "original-name");
        assert!(app.view.status.contains("Edit workspace name"));
    }

    // ── Test 3: confirm_input in RenameSession renames the session ──
    #[test]
    fn confirm_input_renames_session() {
        let mut app = test_app(
            vec![ws("w1", "ws", "/tmp/ws1")],
            vec![sess("abc12345", "old title", "/tmp/ws1")],
        );
        // Set up rename state manually
        app.sessions.tree_state.select(Some(4)); // session is at index 4 (after Pinned + Recent + Workspace + WorkspaceWarning)
        app.view.input_mode = InputMode::RenameSession;
        app.rename_target = Some(0);
        app.input_buffer = "new title".into();

        app.confirm_input().unwrap();

        assert_eq!(app.sessions.sessions[0].title, "new title");
        assert_eq!(app.view.input_mode, InputMode::None);
        assert!(app.input_buffer.is_empty());
        assert!(app.rename_target.is_none());
        assert!(app.view.status.contains("Renamed to: new title"));
    }

    // ── Test 4: confirm_input in SessionName with empty buffer stores None ──
    #[test]
    fn confirm_input_session_name_empty_gives_none() {
        let mut app = test_app(vec![ws("w1", "ws", "/tmp/ws1")], vec![]);
        app.view.input_mode = InputMode::SessionName;
        app.input_buffer = "   ".into(); // whitespace-only
        // With no agents available, it will stay in SessionName but set pending_session_name
        app.available_agents.clear();

        // confirm_input for SessionName with multiple agents goes to SelectAgent.
        // With 0 agents it should still try the branch and not panic.
        let _ = app.confirm_input();

        assert!(app.pending_session_name.is_none());
        assert!(app.input_buffer.is_empty());
    }

    // ── Test 5: start_new_workspace sets correct initial state ──
    #[test]
    fn start_new_workspace_initializes_input_state() {
        let mut app = test_app(vec![], vec![]);
        app.input_buffer = "stale text".into();

        app.start_new_workspace();

        assert_eq!(app.view.input_mode, InputMode::NewWorkspaceName);
        assert!(app.input_buffer.is_empty());
        assert!(app.new_workspace_name.is_none());
        assert!(app.view.status.contains("Workspace name"));
    }
}
