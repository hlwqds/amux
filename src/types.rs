use std::path::{Path, PathBuf};

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
    Gsd,
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

    pub fn build_new_cmd(&self, workspace_path: &Path, session_name: Option<&str>) -> portable_pty::CommandBuilder {
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

    pub fn build_resume_cmd(&self, workspace_path: &Path, session_id: &str) -> portable_pty::CommandBuilder {
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
                let dir =
                    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".claude/projects");
                if dir.exists() { Some(dir) } else { None }
            }
            Agent::Codex => {
                let dir =
                    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".codex/sessions");
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
