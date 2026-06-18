use std::path::{Path, PathBuf};

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Agent {
    Claude,
    Codex,
    Omp,
}

// Manual Ord impl to guarantee fixed sort order: Claude < Codex < Omp
impl Ord for Agent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}
impl PartialOrd for Agent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Agent {
    pub const fn cmd(&self) -> &str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Omp => "omp",
        }
    }

    pub const fn label(&self) -> &str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
            Agent::Omp => "OMP",
        }
    }

    pub fn from_label(label: &str) -> Option<Agent> {
        match label.to_lowercase().as_str() {
            "claude" | "claude code" => Some(Agent::Claude),
            "codex" => Some(Agent::Codex),
            "omp" => Some(Agent::Omp),
            _ => None,
        }
    }

    /// All supported agent types.
    pub const ALL: &[Agent] = &[Agent::Claude, Agent::Codex, Agent::Omp];

    pub const fn icon(&self) -> &str {
        match self {
            Agent::Claude => "C",
            Agent::Codex => "X",
            Agent::Omp => "O",
        }
    }

    pub const fn color(&self) -> Color {
        match self {
            Agent::Claude => Color::Cyan,
            Agent::Codex => Color::Green,
            Agent::Omp => Color::Blue,
        }
    }

    /// Return an actionable install hint if the agent binary is not found.
    pub const fn install_hint(&self) -> &'static str {
        match self {
            Agent::Claude => "Install: npm i -g @anthropic-ai/claude-code",
            Agent::Codex => "Install: npm i -g @openai/codex",
            Agent::Omp => "Install: See omp documentation",
        }
    }

    /// Default environment variables to unset from PTY processes.
    const DEFAULT_UNSET_ENV: &[&str] = &[
        "KITTY_WINDOW_ID",
        "KITTY_LISTEN_ON",
        "TERM_PROGRAM",
        "GHOSTTY_RESOURCES_DIR",
    ];

    pub(crate) fn apply_term_env(cmd: &mut portable_pty::CommandBuilder) {
        cmd.env("TERM", "xterm-256color");
        for var in Self::DEFAULT_UNSET_ENV {
            cmd.env_remove(var);
        }
    }

    pub(crate) fn apply_term_env_with_extra(
        cmd: &mut portable_pty::CommandBuilder,
        extra_unset: &[String],
    ) {
        Self::apply_term_env(cmd);
        for var in extra_unset {
            cmd.env_remove(var);
        }
    }

    pub(crate) fn build_new_cmd(
        &self,
        workspace_path: &Path,
        session_name: Option<&str>,
        unset_env: &[String],
    ) -> portable_pty::CommandBuilder {
        match self {
            Agent::Claude => {
                let mut cmd = portable_pty::CommandBuilder::new("claude");
                cmd.cwd(workspace_path);
                Self::apply_term_env_with_extra(&mut cmd, unset_env);
                if let Some(name) = session_name {
                    cmd.arg("-n");
                    cmd.arg(name);
                }
                cmd
            }
            Agent::Codex => {
                let mut cmd = portable_pty::CommandBuilder::new("codex");
                cmd.cwd(workspace_path);
                Self::apply_term_env_with_extra(&mut cmd, unset_env);
                if let Some(name) = session_name {
                    cmd.arg(name);
                }
                cmd
            }
            Agent::Omp => {
                let mut cmd = portable_pty::CommandBuilder::new("omp");
                cmd.cwd(workspace_path);
                Self::apply_term_env_with_extra(&mut cmd, unset_env);
                // OMP's resize-in-place repaint (alt-screen borrow) is
                // incompatible with amux's nested PTY rendering: it causes
                // Ctrl+O expand (and similar overlays) to flash and revert.
                // Disable it by default; users can override via .amux.json env.
                cmd.env("PI_TUI_RESIZE_IN_PLACE", "0");
                cmd
            }
        }
    }

    pub fn build_resume_cmd(
        &self,
        workspace_path: &Path,
        session_id: &str,
        unset_env: &[String],
    ) -> portable_pty::CommandBuilder {
        match self {
            Agent::Claude => {
                let mut cmd = portable_pty::CommandBuilder::new("claude");
                cmd.cwd(workspace_path);
                Self::apply_term_env_with_extra(&mut cmd, unset_env);
                cmd.arg("--resume");
                cmd.arg(session_id);
                cmd
            }
            Agent::Codex => {
                let mut cmd = portable_pty::CommandBuilder::new("codex");
                cmd.cwd(workspace_path);
                Self::apply_term_env_with_extra(&mut cmd, unset_env);
                cmd.arg("resume");
                cmd.arg(session_id);
                cmd
            }
            Agent::Omp => {
                let mut cmd = portable_pty::CommandBuilder::new("omp");
                cmd.cwd(workspace_path);
                Self::apply_term_env_with_extra(&mut cmd, unset_env);
                cmd.env("PI_TUI_RESIZE_IN_PLACE", "0");
                cmd.arg("--resume");
                cmd.arg(session_id);
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
            Agent::Omp => {
                let dir =
                    PathBuf::from(std::env::var("PI_CODING_AGENT_DIR").unwrap_or_else(|_| {
                        format!("{}/.omp/agent", std::env::var("HOME").unwrap_or_default())
                    }))
                    .join("sessions");
                if dir.exists() { Some(dir) } else { None }
            }
        }
    }
    /// Return the keyboard shortcut character used to select this agent.
    pub const fn shortcut_key(&self) -> char {
        match self {
            Agent::Claude => 'c',
            Agent::Codex => 'x',
            Agent::Omp => 'o',
        }
    }

    pub fn theme_color(&self, theme: &crate::theme::Theme) -> Color {
        match self {
            Agent::Claude => theme.agent_claude,
            Agent::Codex => theme.agent_codex,
            Agent::Omp => theme.agent_omp,
        }
    }
}
