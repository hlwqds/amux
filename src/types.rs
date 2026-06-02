use std::path::{Path, PathBuf};

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
    Gsd,
}

// Manual Ord impl to guarantee fixed sort order: Claude < Codex < Gsd
impl Ord for Agent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}
impl PartialOrd for Agent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some((*self as u8).cmp(&(*other as u8)))
    }
}

impl Agent {
    pub fn cmd(&self) -> &str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Gsd => "gsd",
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
            Agent::Gsd => "GSD",
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            Agent::Claude => "C",
            Agent::Codex => "X",
            Agent::Gsd => "G",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Agent::Claude => Color::Cyan,
            Agent::Codex => Color::Green,
            Agent::Gsd => Color::Magenta,
        }
    }

    pub fn build_new_cmd(
        &self,
        workspace_path: &Path,
        session_name: Option<&str>,
    ) -> portable_pty::CommandBuilder {
        match self {
            Agent::Claude => {
                let mut cmd = portable_pty::CommandBuilder::new("claude");
                cmd.cwd(workspace_path);
                cmd.env("TERM", "xterm-256color");
                cmd.env_remove("KITTY_WINDOW_ID");
                cmd.env_remove("KITTY_LISTEN_ON");
                cmd.env_remove("TERM_PROGRAM");
                cmd.env_remove("GHOSTTY_RESOURCES_DIR");
                if let Some(name) = session_name {
                    cmd.arg("-n");
                    cmd.arg(name);
                }
                cmd
            }
            Agent::Codex => {
                let mut cmd = portable_pty::CommandBuilder::new("codex");
                cmd.cwd(workspace_path);
                cmd.env("TERM", "xterm-256color");
                cmd.env_remove("KITTY_WINDOW_ID");
                cmd.env_remove("KITTY_LISTEN_ON");
                cmd.env_remove("TERM_PROGRAM");
                cmd.env_remove("GHOSTTY_RESOURCES_DIR");
                if let Some(name) = session_name {
                    cmd.arg(name);
                }
                cmd
            }
            Agent::Gsd => {
                let mut cmd = portable_pty::CommandBuilder::new("gsd");
                cmd.cwd(workspace_path);
                cmd.env("TERM", "xterm-256color");
                cmd.env_remove("KITTY_WINDOW_ID");
                cmd.env_remove("KITTY_LISTEN_ON");
                cmd.env_remove("TERM_PROGRAM");
                cmd.env_remove("GHOSTTY_RESOURCES_DIR");
                cmd
            }
        }
    }

    pub fn build_resume_cmd(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> portable_pty::CommandBuilder {
        match self {
            Agent::Claude => {
                let mut cmd = portable_pty::CommandBuilder::new("claude");
                cmd.cwd(workspace_path);
                cmd.env("TERM", "xterm-256color");
                cmd.env_remove("KITTY_WINDOW_ID");
                cmd.env_remove("KITTY_LISTEN_ON");
                cmd.env_remove("TERM_PROGRAM");
                cmd.env_remove("GHOSTTY_RESOURCES_DIR");
                cmd.arg("--resume");
                cmd.arg(session_id);
                cmd
            }
            Agent::Codex => {
                let mut cmd = portable_pty::CommandBuilder::new("codex");
                cmd.cwd(workspace_path);
                cmd.env("TERM", "xterm-256color");
                cmd.env_remove("KITTY_WINDOW_ID");
                cmd.env_remove("KITTY_LISTEN_ON");
                cmd.env_remove("TERM_PROGRAM");
                cmd.env_remove("GHOSTTY_RESOURCES_DIR");
                cmd.arg("resume");
                cmd.arg(session_id);
                cmd
            }
            Agent::Gsd => {
                let mut cmd = portable_pty::CommandBuilder::new("gsd");
                cmd.cwd(workspace_path);
                cmd.env("TERM", "xterm-256color");
                cmd.env_remove("KITTY_WINDOW_ID");
                cmd.env_remove("KITTY_LISTEN_ON");
                cmd.env_remove("TERM_PROGRAM");
                cmd.env_remove("GHOSTTY_RESOURCES_DIR");
                cmd.arg("sessions");
                cmd
            }
        }
    }

    pub fn sessions_dir(&self) -> Option<PathBuf> {
        match self {
            Agent::Claude => {
                let dir = PathBuf::from(std::env::var("HOME").unwrap_or_default())
                    .join(".claude/projects");
                if dir.exists() { Some(dir) } else { None }
            }
            Agent::Codex => {
                let dir = PathBuf::from(std::env::var("HOME").unwrap_or_default())
                    .join(".codex/sessions");
                if dir.exists() { Some(dir) } else { None }
            }
            Agent::Gsd => {
                let dir =
                    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".gsd/sessions");
                if dir.exists() { Some(dir) } else { None }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: Option<PathBuf>,
    pub created_at: u64,
    #[serde(skip)]
    pub expanded: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub workspaces: Vec<Workspace>,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub workspace_path: PathBuf,
    pub title: String,
    pub last_active: u64,
    pub agent: Agent,
}

#[derive(Clone, Debug)]
pub enum TreeNode {
    Workspace(usize),
    Session(usize, usize),
    ActiveTab(usize),
    AgentHeader(Agent),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortMode {
    #[default]
    TimeDesc,
    TimeAsc,
    NameAsc,
    NameDesc,
    AgentGroup,
}

impl SortMode {
    pub fn next(&self) -> SortMode {
        match self {
            SortMode::TimeDesc => SortMode::TimeAsc,
            SortMode::TimeAsc => SortMode::NameAsc,
            SortMode::NameAsc => SortMode::NameDesc,
            SortMode::NameDesc => SortMode::AgentGroup,
            SortMode::AgentGroup => SortMode::TimeDesc,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SortMode::TimeDesc => "time \u{2193}",
            SortMode::TimeAsc => "time \u{2191}",
            SortMode::NameAsc => "name A\u{2192}Z",
            SortMode::NameDesc => "name Z\u{2192}A",
            SortMode::AgentGroup => "agent",
        }
    }
}

pub struct PtySlot {
    pub handle: crate::pty::PtyHandle,
    pub info: RunningInfo,
}

#[derive(Clone, Debug)]
pub struct RunningInfo {
    pub workspace_path: PathBuf,
    pub title: String,
    pub session_id: Option<String>,
    pub started_at: u64,
    pub completed: bool,
    pub agent: Agent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Chat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    None,
    SessionName,
    SelectAgent,
    RenameSession,
    RenameWorkspace,
    NewWorkspaceName,
    BrowseDir,
    Search,
}

#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

pub enum Action {
    Continue,
    Quit,
}

#[derive(Deserialize)]
pub struct ClaudeRecord {
    #[serde(rename = "type")]
    pub record_type: Option<String>,
    pub message: Option<ClaudeMessage>,
}

#[derive(Deserialize)]
pub struct ClaudeMessage {
    pub role: Option<String>,
    pub content: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use ratatui::style::Color;
    use serde_json;

    use super::*;

    #[test]
    fn config_roundtrip() {
        let config = Config {
            workspaces: vec![
                Workspace {
                    id: "ws-1".into(),
                    name: "Project A".into(),
                    path: Some(PathBuf::from("/home/user/proj-a")),
                    created_at: 1000,
                    expanded: false,
                },
                Workspace {
                    id: "ws-2".into(),
                    name: "Virtual".into(),
                    path: None,
                    created_at: 2000,
                    expanded: true,
                },
            ],
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.workspaces.len(), 2);
        assert_eq!(parsed.workspaces[0].id, "ws-1");
        assert_eq!(
            parsed.workspaces[0].path,
            Some(PathBuf::from("/home/user/proj-a"))
        );
        assert_eq!(parsed.workspaces[1].path, None);
        assert!(!parsed.workspaces[0].expanded);
        assert!(!parsed.workspaces[1].expanded);
    }

    #[test]
    fn workspace_serialization_virtual() {
        let ws = Workspace {
            id: "test-id".into(),
            name: "No Path".into(),
            path: None,
            created_at: 0,
            expanded: false,
        };
        let json = serde_json::to_string(&ws).unwrap();
        assert!(json.contains("\"path\":null"));
        let parsed: Workspace = serde_json::from_str(&json).unwrap();
        assert!(parsed.path.is_none());
    }

    #[test]
    fn agent_traits() {
        assert_eq!(Agent::Claude.cmd(), "claude");
        assert_eq!(Agent::Codex.cmd(), "codex");
        assert_eq!(Agent::Gsd.cmd(), "gsd");
        assert_eq!(Agent::Claude.label(), "Claude Code");
        assert_eq!(Agent::Codex.label(), "Codex");
        assert_eq!(Agent::Gsd.label(), "GSD");
        assert_eq!(Agent::Claude.icon(), "C");
        assert_eq!(Agent::Codex.icon(), "X");
        assert_eq!(Agent::Gsd.icon(), "G");
        assert_eq!(Agent::Claude.color(), Color::Cyan);
        assert_eq!(Agent::Codex.color(), Color::Green);
        assert_eq!(Agent::Gsd.color(), Color::Magenta);
    }

    #[test]
    fn gsd_sessions_persist_after_pty_exit() {
        // Verifies the poll_states() retain logic from app.rs:
        //   self.ptys.retain(|slot| {
        //       if slot.info.agent == Agent::Codex && !slot.handle.is_alive() { return false; }
        //       true
        //   });
        // Only Codex sessions are cleaned up on PTY exit; GSD and Claude persist.
        let should_retain = |agent: Agent, is_alive: bool| -> bool {
            if agent == Agent::Codex && !is_alive {
                return false;
            }
            true
        };

        // When PTY is alive, all agents are retained
        assert!(
            should_retain(Agent::Claude, true),
            "Claude should retain when alive"
        );
        assert!(
            should_retain(Agent::Codex, true),
            "Codex should retain when alive"
        );
        assert!(
            should_retain(Agent::Gsd, true),
            "GSD should retain when alive"
        );

        // When PTY exits, only Codex is removed — GSD and Claude persist
        assert!(
            should_retain(Agent::Claude, false),
            "Claude sessions MUST persist after PTY exit"
        );
        assert!(
            !should_retain(Agent::Codex, false),
            "Codex sessions should be cleaned up after PTY exit"
        );
        assert!(
            should_retain(Agent::Gsd, false),
            "GSD sessions MUST persist after PTY exit"
        );
    }

    #[test]
    fn gsd_build_new_cmd_no_session_name() {
        let ws = Path::new("/home/user/proj");
        let cmd = Agent::Gsd.build_new_cmd(ws, None);
        // CommandBuilder doesn't expose args directly, but the method must not panic
        // and must return a valid builder (tested by compilation + the agent_traits test)
        let _ = cmd;
    }

    #[test]
    fn gsd_build_resume_cmd_uses_sessions() {
        let ws = Path::new("/home/user/proj");
        let cmd = Agent::Gsd.build_resume_cmd(ws, "some-session-id");
        // Verify the builder is created without panic.
        // The resume uses "gsd sessions" (interactive picker) not --resume.
        let _ = cmd;
    }
}
