use anyhow::{Context, Result};

use crate::config::save_session_title;
use crate::pty::PtyHandle;
use crate::types::*;
use crate::util::*;

impl super::App {
    fn spawn_session(&mut self, agent: Agent) -> Result<()> {
        let name = self.pending_session_name.take();
        self.spawn_with_agent(agent, name)
    }

    pub(super) fn spawn_with_agent(&mut self, agent: Agent, name: Option<String>) -> Result<()> {
        let chat_size = self.chat_size();

        match self.selected_node().cloned() {
            Some(TreeNode::Workspace(wi)) => {
                let path = self.workspace_cwd(wi);
                let display_name = name.clone().unwrap_or_else(|| "unnamed".into());
                self.status = format!(
                    "Starting {} '{}' in {}...",
                    agent.label(),
                    display_name,
                    self.workspaces[wi].name
                );
                let pty = PtyHandle::spawn(agent, &path, None, name.as_deref(), chat_size)
                    .context(format!("failed to spawn {}", agent.label()))?;
                let idx = self.ptys.len();
                self.ptys.push(PtySlot {
                    handle: pty,
                    info: RunningInfo {
                        workspace_path: path,
                        title: display_name,
                        session_id: None,
                        started_at: now_secs(),
                        completed: false,
                        agent,
                    },
                });
                self.active_pty = Some(idx);
                self.focus = Focus::Chat;
                self.rebuild_tree();
            }
            Some(TreeNode::Session(_wi, si)) => {
                let session = self.sessions[si].clone();
                if let Some(existing) = self.pty_index_for_session(&session.id) {
                    self.active_pty = Some(existing);
                    self.focus = Focus::Chat;
                    return Ok(());
                }
                let path = session.workspace_path.clone();
                let id = session.id.clone();
                let title = session.title.clone();
                self.status = format!("Resuming: {}...", &id[..8.min(id.len())]);
                let pty = PtyHandle::spawn(agent, &path, Some(&id), name.as_deref(), chat_size)
                    .context("failed to resume session")?;
                let idx = self.ptys.len();
                self.ptys.push(PtySlot {
                    handle: pty,
                    info: RunningInfo {
                        workspace_path: path,
                        title,
                        session_id: Some(id),
                        started_at: now_secs(),
                        completed: false,
                        agent,
                    },
                });
                self.active_pty = Some(idx);
                self.focus = Focus::Chat;
                self.rebuild_tree();
            }
            Some(TreeNode::ActiveTab(pi)) => {
                self.active_pty = Some(pi);
                self.focus = Focus::Chat;
            }
            None => {}
        }
        Ok(())
    }


    pub(super) fn confirm_input(&mut self) -> Result<()> {
        match self.input_mode {
            InputMode::SessionName => {
                let name = if self.input_buffer.trim().is_empty() {
                    None
                } else {
                    Some(self.input_buffer.clone())
                };
                self.pending_session_name = name;
                self.input_buffer.clear();
                if self.available_agents.len() == 1 {
                    let agent = self.available_agents[0];
                    self.input_mode = InputMode::None;
                    self.spawn_session(agent)?;
                } else {
                    self.input_mode = InputMode::SelectAgent;
                    self.agent_state.select(Some(0));
                    self.status =
                        "Select agent \u{00b7} Enter to confirm \u{00b7} Esc to cancel".into();
                }
            }
            InputMode::RenameSession => {
                if let Some(si) = self.rename_target {
                    let new_title = if self.input_buffer.trim().is_empty() {
                        self.sessions[si].id[..8.min(self.sessions[si].id.len())].to_string()
                    } else {
                        self.input_buffer.clone()
                    };
                    let _ = save_session_title(&self.sessions[si].id, &new_title);
                    self.sessions[si].title = new_title.clone();
                    self.status = format!("Renamed to: {}", new_title);
                    self.rebuild_tree();
                }
                self.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.rename_target = None;
            }
            InputMode::NewWorkspaceName => {
                let name = self.input_buffer.trim().to_string();
                if name.is_empty() {
                    self.status = "Workspace name cannot be empty.".into();
                    self.input_mode = InputMode::None;
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
                        self.workspaces[wi].name.clone()
                    } else {
                        self.input_buffer.clone()
                    };
                    self.workspaces[wi].name = new_name.clone();
                    self.save_config();
                    self.status = format!("Workspace renamed to: {}", new_name);
                    self.rebuild_tree();
                }
                self.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.rename_workspace_target = None;
            }
            InputMode::SelectAgent => {
                if let Some(idx) = self.agent_state.selected()
                    && let Some(&agent) = self.available_agents.get(idx)
                {
                    self.input_mode = InputMode::None;
                    self.spawn_session(agent)?;
                    return Ok(());
                }
                self.input_mode = InputMode::None;
            }
            InputMode::None | InputMode::BrowseDir => {}
        }
        Ok(())
    }

    pub(super) fn start_rename(&mut self) {
        match self.selected_node().cloned() {
            Some(TreeNode::Workspace(wi)) if wi < self.workspaces.len() => {
                self.input_mode = InputMode::RenameWorkspace;
                self.rename_workspace_target = Some(wi);
                self.input_buffer = self.workspaces[wi].name.clone();
                self.status = "Edit workspace name (Enter=confirm, Esc=cancel):".into();
            }
            Some(TreeNode::Session(_, si)) if si < self.sessions.len() => {
                self.input_mode = InputMode::RenameSession;
                self.rename_target = Some(si);
                self.input_buffer = self.sessions[si].title.clone();
                self.status = "Edit session name (Enter=confirm, Esc=cancel):".into();
            }
            _ => {}
        }
    }

    pub(super) fn start_new_workspace(&mut self) {
        self.input_mode = InputMode::NewWorkspaceName;
        self.input_buffer.clear();
        self.new_workspace_name = None;
        self.status = "Workspace name (Esc = cancel):".into();
    }

}
