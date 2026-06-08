use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::agent::Agent;
use super::keybind::Keybinds;

const fn default_true() -> bool {
    true
}

/// Per-project configuration loaded from `.amux.json` in the workspace root.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    #[serde(default)]
    pub default_agent: Option<String>,
    #[serde(default)]
    pub default_template: Option<String>,
    #[serde(default)]
    pub check_command: Option<String>,
    #[serde(default)]
    pub ignore_sessions: Vec<String>,
    #[serde(default)]
    pub env: Vec<(String, String)>,
    #[serde(default = "default_true")]
    pub auto_inject_knowledge: bool,
    #[serde(default)]
    pub preflight: PreflightConfig,
}

/// Configuration for pre-flight checks before starting a session.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PreflightConfig {
    #[serde(default)]
    pub require_clean_git: bool,
    #[serde(default)]
    pub mode: PreflightMode,
}

/// How to display pre-flight check results.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PreflightMode {
    #[default]
    Popup,
    Silent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: Option<PathBuf>,
    pub created_at: u64,
    /// Session IDs that belong to this workspace. Only these are shown.
    /// Populated manually (spawn) or via scan keybind (Alt+Shift+R).
    #[serde(default)]
    pub session_ids: Vec<String>,
    #[serde(default)]
    pub expanded: bool,
}

/// A remote host for SSH-based session discovery.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteHost {
    pub name: String,
    pub host: String,
    /// Optional SSH user.
    #[serde(default)]
    pub user: Option<String>,
    /// Optional SSH port.
    #[serde(default)]
    pub port: Option<u16>,
    /// Custom paths on the remote host to scan for session JSONL files.
    /// Defaults are used when empty.
    #[serde(default)]
    pub agent_paths: Vec<String>,
}

/// A user-defined plugin command.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    /// Shell command to execute. {workspace} and {session_id} are replaced.
    pub command: String,
    /// Optional single-char key binding.
    #[serde(default)]
    pub key: Option<char>,
    /// Hook events this plugin should fire on (e.g. "on_complete", "on_idle").
    #[serde(default)]
    pub hooks: Vec<String>,
}

/// Actions a plugin can trigger via JSON output.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "action")]
pub enum PluginAction {
    #[serde(rename = "create_session")]
    CreateSession {
        agent: Option<String>,
        prompt: Option<String>,
    },
    #[serde(rename = "switch_workspace")]
    SwitchWorkspace { id: Option<String> },
    #[serde(rename = "notify")]
    Notify { message: String },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub workspaces: Vec<Workspace>,
    /// Config schema version. Increment when making breaking changes.
    /// Used by load_config to apply migrations.
    #[serde(default)]
    pub config_version: u32,
    #[serde(default)]
    pub theme: crate::theme::ThemeName,
    #[serde(default)]
    pub keybinds: Keybinds,
    #[serde(default)]
    pub templates: Vec<SessionTemplate>,
    #[serde(default)]
    pub automations: Vec<InputAutomation>,
    /// Days after which sessions are considered old enough to archive. None = no auto-archive.
    #[serde(default)]
    pub archive_days: Option<u64>,
    /// Remote hosts for SSH-based session discovery.
    #[serde(default)]
    pub remote_hosts: Vec<RemoteHost>,
    /// User-defined plugin commands.
    #[serde(default)]
    pub plugins: Vec<Plugin>,
    /// Port for the built-in HTTP server (default: 8080). None = use default.
    #[serde(default)]
    pub serve_port: Option<u16>,
    /// Bearer token for HTTP server auth. None = no auth.
    #[serde(default)]
    pub serve_token: Option<String>,
    /// Override the auto-detected check command. Format: "command arg1 arg2"
    #[serde(default)]
    pub check_command: Option<String>,
    /// Token budget alerts. Set daily/weekly limits for tokens and cost.
    #[serde(default)]
    pub token_budget: Option<crate::budget::TokenBudget>,
    /// Session chains: named sequences of agent steps with prompt templates.
    #[serde(default)]
    pub chains: Vec<crate::chain::SessionChain>,
    /// Environment variables to unset from child PTY processes.
    /// Defaults to terminal multiplexer vars (KITTY_WINDOW_ID, etc).
    #[serde(default)]
    pub unset_env: Vec<String>,
    /// Whether the "Recent" virtual workspace is expanded in sidebar.
    #[serde(default)]
    pub recent_expanded: bool,
    /// Whether the "Pinned" virtual workspace is expanded in sidebar.
    #[serde(default)]
    pub pinned_expanded: bool,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub workspace_path: PathBuf,
    pub title: String,
    pub last_active: u64,
    pub agent: Agent,
    pub tags: Vec<String>,
    pub pinned: bool,
    /// Last user message text (truncated), extracted from JSONL during discovery.
    pub last_message: Option<String>,
}

/// A saved session template for quick launch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionTemplate {
    pub name: String,
    pub agent: Agent,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub initial_prompt: Option<String>,
}

/// A single step in an input automation sequence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputStep {
    /// Text to send (newline appended automatically).
    pub text: String,
    /// Delay in milliseconds before sending this step.
    #[serde(default)]
    pub delay_ms: u64,
}

/// A saved input automation sequence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputAutomation {
    pub name: String,
    pub steps: Vec<InputStep>,
}

/// A pending input step awaiting delivery to a PTY.
#[derive(Clone, Debug)]
pub struct PendingInput {
    /// Monotonic millis when this step should fire.
    pub fire_at_ms: u64,
    /// The text to send (newline appended).
    pub text: String,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::style::Color;
    use serde_json;

    use super::*;
    use crate::types::{Agent, KeyBinding, Keybinds, SortMode};

    #[test]
    fn config_roundtrip() {
        let config = Config {
            workspaces: vec![
                Workspace {
                    id: "ws-1".into(),
                    name: "Project A".into(),
                    path: Some(PathBuf::from("/home/user/proj-a")),
                    created_at: 1000,
                    session_ids: Vec::new(),
                    expanded: false,
                },
                Workspace {
                    id: "ws-2".into(),
                    name: "Virtual".into(),
                    path: None,
                    created_at: 2000,
                    session_ids: Vec::new(),
                    expanded: true,
                },
            ],
            theme: crate::theme::ThemeName::Dark,
            ..Default::default()
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
        assert!(parsed.workspaces[1].expanded);
    }

    #[test]
    fn status_hint_stays_compact_for_footer() {
        let hint = Keybinds::default().status_hint();

        assert!(
            hint.len() <= 64,
            "hint should fit narrow status bars: {hint}"
        );
        assert!(hint.contains("Enter:open"));
        assert!(hint.contains("Tab:focus"));
        assert!(hint.contains("Alt+k:help"));
        assert!(!hint.contains("rename"));
        assert!(!hint.contains("open dir"));
    }

    #[test]
    fn workspace_serialization_virtual() {
        let ws = Workspace {
            id: "test-id".into(),
            name: "No Path".into(),
            path: None,
            created_at: 0,
            session_ids: Vec::new(),
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
        assert_eq!(Agent::Claude.label(), "Claude Code");
        assert_eq!(Agent::Codex.label(), "Codex");
        assert_eq!(Agent::Claude.icon(), "C");
        assert_eq!(Agent::Codex.icon(), "X");
        assert_eq!(Agent::Claude.color(), Color::Cyan);
        assert_eq!(Agent::Codex.color(), Color::Green);
    }

    #[test]
    fn project_config_default_is_empty() {
        let config = ProjectConfig::default();
        assert!(config.default_agent.is_none());
        assert!(config.default_template.is_none());
        assert!(config.check_command.is_none());
        assert!(config.ignore_sessions.is_empty());
        assert!(config.env.is_empty());
        // auto_inject_knowledge defaults to false in Default impl, true via serde
    }

    #[test]
    fn project_config_roundtrip() {
        let config = ProjectConfig {
            default_agent: Some("claude".into()),
            default_template: None,
            check_command: Some("npm test".into()),
            ignore_sessions: vec!["temp-".into()],
            env: vec![("NODE_ENV".into(), "development".into())],
            auto_inject_knowledge: true,
            preflight: PreflightConfig::default(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ProjectConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.default_agent, Some("claude".to_string()));
        assert_eq!(parsed.check_command, Some("npm test".to_string()));
        assert_eq!(parsed.ignore_sessions, vec!["temp-"]);
        assert_eq!(parsed.env, vec![("NODE_ENV".into(), "development".into())]);
    }

    // --- Agent::from_label ---

    #[test]
    fn agent_from_label_known_agents() {
        assert_eq!(Agent::from_label("claude"), Some(Agent::Claude));
        assert_eq!(Agent::from_label("Claude"), Some(Agent::Claude));
        assert_eq!(Agent::from_label("CLAUDE"), Some(Agent::Claude));
        assert_eq!(Agent::from_label("claude code"), Some(Agent::Claude));
        assert_eq!(Agent::from_label("Claude Code"), Some(Agent::Claude));
        assert_eq!(Agent::from_label("CLAUDE CODE"), Some(Agent::Claude));
        assert_eq!(Agent::from_label("codex"), Some(Agent::Codex));
        assert_eq!(Agent::from_label("Codex"), Some(Agent::Codex));
        assert_eq!(Agent::from_label("omp"), Some(Agent::Omp));
        assert_eq!(Agent::from_label("OMP"), Some(Agent::Omp));
    }

    #[test]
    fn agent_from_label_unknown_returns_none() {
        assert_eq!(Agent::from_label("unknown"), None);
        assert_eq!(Agent::from_label(""), None);
        assert_eq!(Agent::from_label("claudecode"), None);
        assert_eq!(Agent::from_label("copilot"), None);
    }

    // --- Agent::ALL ---

    #[test]
    fn agent_all_contains_every_variant() {
        assert_eq!(Agent::ALL.len(), 3);
        assert!(Agent::ALL.contains(&Agent::Claude));
        assert!(Agent::ALL.contains(&Agent::Codex));
        assert!(Agent::ALL.contains(&Agent::Omp));
        // Verify fixed sort order: Claude < Codex < Omp
        assert!(Agent::ALL.windows(2).all(|w| w[0] < w[1]));
    }

    // --- SortMode::next ---

    #[test]
    fn sort_mode_next_cycles_through_all_variants() {
        use SortMode::*;
        let variants = [TimeDesc, TimeAsc, NameAsc, NameDesc, AgentGroup];
        // Verify each step advances correctly
        for i in 0..variants.len() {
            assert_eq!(variants[i].next(), variants[(i + 1) % variants.len()]);
        }
        // Full cycle returns to start
        let mut mode = TimeDesc;
        for _ in 0..variants.len() {
            mode = mode.next();
        }
        assert_eq!(mode, TimeDesc);
    }

    // --- KeyBinding::matches_event ---

    #[test]
    fn keybinding_matches_event_various_keys() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        // Plain char
        let kb = KeyBinding::key("q");
        assert!(kb.matches_event(&KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)));
        assert!(!kb.matches_event(&KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)));

        // Ctrl+char
        let kb_ctrl = KeyBinding::ctrl("c");
        assert!(kb_ctrl.matches_event(&KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(!kb_ctrl.matches_event(&KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)));

        // Alt+char
        let kb_alt = KeyBinding::alt("k");
        assert!(kb_alt.matches_event(&KeyEvent::new(KeyCode::Char('k'), KeyModifiers::ALT)));

        // Special keys
        let kb_enter = KeyBinding::key("enter");
        assert!(kb_enter.matches_event(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));

        let kb_f5 = KeyBinding::key("f5");
        assert!(kb_f5.matches_event(&KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)));
        assert!(!kb_f5.matches_event(&KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE)));

        // Modifiers mismatch
        let kb_shift = KeyBinding::shift("up");
        assert!(kb_shift.matches_event(&KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT)));
        assert!(!kb_shift.matches_event(&KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)));
    }

    // --- Keybinds::validate ---

    #[test]
    fn keybinds_validate_no_conflicts_by_default() {
        let kb = Keybinds::default();
        assert!(
            kb.validate().is_empty(),
            "default keybinds should have no conflicts"
        );
    }

    #[test]
    fn keybinds_validate_detects_duplicate_bindings() {
        let mut kb = Keybinds::default();
        // Force a conflict: set move_up identical to move_down
        kb.move_up = kb.move_down.clone();
        let conflicts = kb.validate();
        assert!(
            !conflicts.is_empty(),
            "duplicate binding should produce a conflict"
        );
        assert!(
            conflicts
                .iter()
                .any(|(a, b)| (*a == "move_up" && *b == "move_down")
                    || (*a == "move_down" && *b == "move_up"))
        );
    }
}
