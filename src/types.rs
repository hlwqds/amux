use std::path::{Path, PathBuf};

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
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
    pub fn cmd(&self) -> &str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Omp => "omp",
        }
    }

    pub fn label(&self) -> &str {
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

    pub fn icon(&self) -> &str {
        match self {
            Agent::Claude => "C",
            Agent::Codex => "X",
            Agent::Omp => "O",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Agent::Claude => Color::Cyan,
            Agent::Codex => Color::Green,
            Agent::Omp => Color::Blue,
        }
    }

    /// Return an actionable install hint if the agent binary is not found.
    pub fn install_hint(&self) -> &'static str {
        match self {
            Agent::Claude => "Install: npm i -g @anthropic-ai/claude-code",
            Agent::Codex => "Install: npm i -g @openai/codex",
            Agent::Omp => "Install: See omp documentation",
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
            Agent::Omp => {
                let mut cmd = portable_pty::CommandBuilder::new("omp");
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
            Agent::Omp => {
                let mut cmd = portable_pty::CommandBuilder::new("omp");
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub workspaces: Vec<Workspace>,
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
}

#[derive(Clone, Debug)]
pub enum TreeNode {
    Workspace(usize),
    /// Virtual workspace for pinned sessions (contains session indices).
    PinnedWorkspace,
    /// Warning about a workspace (e.g. path not found). Contains (workspace_index, message).
    WorkspaceWarning(usize, String),
    Session(usize, usize),
    ActiveTab(usize),
    AgentHeader(Agent),
    ArchivedHeader,
    ArchivedSession(usize, usize),
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
#[derive(Clone)]
pub struct PtySlot {
    pub id: String,
    pub handle: crate::pty::PtyHandle,
    pub info: RunningInfo,
    /// Last recorded screen content hash for change detection.
    pub last_screen_hash: u64,
    /// Timestamp of last recording frame write (for throttling).
    pub last_recording_at: std::time::Instant,
    /// Resource usage stats from /proc, updated on 30s interval.
    pub process_stats: Option<crate::procfs::ProcessStats>,
}

/// Git state recorded when a session completes.
#[derive(Clone, Debug, Default)]
pub struct GitInfo {
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub diff_stat: Option<String>,
}

/// Diff summary generated when a session completes.
#[derive(Clone, Debug, Default)]
pub struct DiffSummary {
    pub files_changed: Vec<String>,
    pub insertions: u32,
    pub deletions: u32,
    pub summary_line: Option<String>,
}

/// Result of post-completion check (cargo test/clippy).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CheckStatus {
    Pending,
    Running,
    Passed,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct RunningInfo {
    pub workspace_path: PathBuf,
    pub title: String,
    pub session_id: Option<String>,
    pub started_at: u64,
    pub completed: bool,
    pub agent: Agent,
    pub git_info: GitInfo,
    pub check_status: CheckStatus,
    pub diff_summary: DiffSummary,
    pub project_type: crate::discovery::ProjectType,
    /// Git worktree branch name if this session was isolated due to file conflicts.
    pub worktree_branch: Option<String>,
    /// Git HEAD commit hash captured when the session was spawned (for rollback).
    pub snapshot_commit: Option<String>,
}

/// Per-agent activity statistics for the stats popup.
#[derive(Clone, Debug)]
pub struct AgentStats {
    pub agent: Agent,
    pub total_sessions: usize,
    pub active_sessions: usize,
    pub completed_sessions: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Focus {
    #[default]
    Sidebar,
    Chat,
}

// EVALUATION: InputMode consolidation
// ─────────────────────────────────────────────
// Current state: 36 variants (including `None`), flat C-like enum with `Copy + Clone`.
// Dead variant: `DiffSelect` exists but is never used anywhere.
//
// ## Option A — Categorical grouping (TextInput(TextInputKind), ListSelect(…), Popup(…), Confirm(…))
//
// Handler dispatch (handler.rs:481-748) uses ~30 sequential `if self.input_mode == Variant`
// comparisons, each delegating to a dedicated method. Grouping into categories would NOT
// collapse these branches because each variant has unique behavior — even within the same
// "category", no two TextInput modes share confirm logic (SessionName → agent select,
// RenameSession → save title, NewWorkspaceName → browse dir, Search → filter tree).
//
// The one genuine pattern: 8+ variants (Help, Stats, TokenStats, CrossSearch, DiffView,
// AgentRecommend, Timeline, ConflictWarning, BudgetWarning, KeybindView, SummaryPreview)
// all close on "any key" with identical `self.input_mode = InputMode::None; return Continue`.
// This could be a `Popup(PopupKind)` arm with a shared `dismiss_any_key` handler. Savings:
// ~30 lines of dispatch boilerplate. But each still needs its own render function and
// cleanup logic (e.g., `self.diff_lines.clear()`, `self.timeline_events.clear()`), so the
// dismiss handler would need a `match PopupKind` anyway — just moved.
//
// UI render dispatch (ui.rs:42-88) is a 22-arm if-else chain. Each arm calls a unique
// render method. Grouping into `Popup(PopupKind)` would replace 12 `else if` lines with
// one `Popup` arm containing a 12-way match inside `render_popup()` — same total branches.
//
// `confirm_input` (session.rs:202-289) handles 5 modes with distinct logic and falls through
// the remaining 31 as no-ops. Categorical grouping wouldn't reduce this — each TextInput
// variant does something completely different on confirm.
//
// ## Option B — Data-carrying enum (embed buffer/state in variants)
//
// This would break `Copy` on InputMode. Every `self.input_mode == InputMode::Foo` comparison
// throughout the codebase (30+ sites) currently works because the enum is `Copy + PartialEq`.
// Adding `String` or `ListState` fields means:
//   - Loss of `Copy` → every comparison needs `&self.input_mode` or destructuring
//   - Buffer state duplicated: `input_buffer` already lives on `App` alongside mode-specific
//     state (agent_state, browse_state, template_state, etc.). Moving it INTO InputMode
//     creates two sources of truth and complicates transitions.
//   - `confirm_input()` already reads `self.input_buffer`; if buffer were in the enum,
//     we'd need to destructure `self.input_mode` mutably while also accessing `self.sessions`,
//     triggering borrow conflicts.
// Net: significant API churn for no behavioral improvement.
//
// ## Option C — Keep as-is (RECOMMENDED)
//
// The flat enum is:
//   1. Zero-cost: `Copy + PartialEq`, compares as a simple integer discriminant.
//   2. Explicit: every mode is greppable, no indirection through sub-enums.
//   3. Already well-factored: handler delegates to per-mode methods, UI to per-mode renders.
//      The flat dispatch is a thin routing layer, not a complexity sink.
//   4. Easy to extend: adding a new mode = add one variant + one handler + one render.
//      Categorical grouping adds a step: pick the right category first.
//
// The only actionable cleanup: remove the dead `DiffSelect` variant.
//
// Recommendation: **Keep as-is** (remove DiffSelect).
// Rationale: The flat enum correctly models the domain — each variant IS a distinct UI mode
// with unique key handling, rendering, and confirmation logic. Categorical grouping would
// redistribute the same match arms across more types without reducing total complexity.
// Data-carrying would sacrifice Copy and create borrow conflicts for no gain. The flat enum
// is the simplest representation that handles all 36 cases correctly.
// ─────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputMode {
    #[default]
    None,
    SessionName,
    SelectAgent,
    RenameSession,
    RenameWorkspace,
    NewWorkspaceName,
    BrowseDir,
    Search,
    ConfirmDelete,
    Help,
    SessionPreview,
    TagFilter,
    Settings,
    TemplateSelect,
    AutomationSelect,
    BranchSelect,
    Stats,
    TokenStats,
    DiffSelect,
    DiffView,
    RemoteView,
    PluginList,
    PluginOutput,
    Timeline,
    ConflictWarning,
    ConflictResolve,
    AgentRecommend,
    CrossSearch,
    SummaryPreview,
    KeybindView,
    ThemeSelect,
    BudgetWarning,
    RollbackConfirm,
    ChainSelect,
    PreflightConfirm,
    SemanticSearch,
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

/// A user-configurable key binding.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

impl KeyBinding {
    pub fn key(key: &str) -> Self {
        KeyBinding {
            key: key.into(),
            ctrl: false,
            shift: false,
            alt: false,
        }
    }
    pub fn ctrl(key: &str) -> Self {
        KeyBinding {
            key: key.into(),
            ctrl: true,
            shift: false,
            alt: false,
        }
    }
    pub fn shift(key: &str) -> Self {
        KeyBinding {
            key: key.into(),
            ctrl: false,
            shift: true,
            alt: false,
        }
    }
    pub fn alt(key: &str) -> Self {
        KeyBinding {
            key: key.into(),
            ctrl: false,
            shift: false,
            alt: true,
        }
    }

    /// Check if a KeyEvent matches this binding.
    pub fn matches_event(&self, key: &crossterm::event::KeyEvent) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};
        let mods_match = key.modifiers.contains(KeyModifiers::CONTROL) == self.ctrl
            && key.modifiers.contains(KeyModifiers::SHIFT) == self.shift
            && key.modifiers.contains(KeyModifiers::ALT) == self.alt;
        if !mods_match {
            return false;
        }
        match &key.code {
            KeyCode::Char(c) => self.key == c.to_string(),
            KeyCode::Enter => self.key == "enter",
            KeyCode::Esc => self.key == "esc",
            KeyCode::Up => self.key == "up",
            KeyCode::Down => self.key == "down",
            KeyCode::Backspace => self.key == "backspace",
            KeyCode::Tab => self.key == "tab",
            KeyCode::F(n) => self.key == format!("f{}", n),
            _ => false,
        }
    }
    pub fn display(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push_str("Ctrl+");
        }
        if self.alt {
            s.push_str("Alt+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        s.push_str(&self.key);
        s
    }
}

/// All configurable key bindings with defaults.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keybinds {
    #[serde(default = "Keybinds::default_move_up")]
    pub move_up: KeyBinding,
    #[serde(default = "Keybinds::default_move_down")]
    pub move_down: KeyBinding,
    #[serde(default = "Keybinds::default_expand")]
    pub expand: KeyBinding,
    #[serde(default = "Keybinds::default_refresh")]
    pub refresh: KeyBinding,
    #[serde(default = "Keybinds::default_rename")]
    pub rename: KeyBinding,
    #[serde(default = "Keybinds::default_new_workspace")]
    pub new_workspace: KeyBinding,
    #[serde(default = "Keybinds::default_delete")]
    pub delete: KeyBinding,
    #[serde(default = "Keybinds::default_new_session")]
    pub new_session: KeyBinding,
    #[serde(default = "Keybinds::default_search")]
    pub search: KeyBinding,
    #[serde(default = "Keybinds::default_help")]
    pub help: KeyBinding,
    #[serde(default = "Keybinds::default_settings")]
    pub settings: KeyBinding,
    #[serde(default = "Keybinds::default_theme")]
    pub theme: KeyBinding,
    #[serde(default = "Keybinds::default_export")]
    pub export: KeyBinding,
    #[serde(default = "Keybinds::default_copy")]
    pub copy: KeyBinding,
    #[serde(default = "Keybinds::default_preview")]
    pub preview: KeyBinding,
    #[serde(default = "Keybinds::default_tag_filter")]
    pub tag_filter: KeyBinding,
    #[serde(default = "Keybinds::default_quit")]
    pub quit: KeyBinding,
}

impl Default for Keybinds {
    fn default() -> Self {
        Keybinds {
            move_up: Keybinds::default_move_up(),
            move_down: Keybinds::default_move_down(),
            expand: Keybinds::default_expand(),
            refresh: Keybinds::default_refresh(),
            rename: Keybinds::default_rename(),
            new_workspace: Keybinds::default_new_workspace(),
            delete: Keybinds::default_delete(),
            new_session: Keybinds::default_new_session(),
            search: Keybinds::default_search(),
            help: Keybinds::default_help(),
            settings: Keybinds::default_settings(),
            theme: Keybinds::default_theme(),
            export: Keybinds::default_export(),
            copy: Keybinds::default_copy(),
            preview: Keybinds::default_preview(),
            tag_filter: Keybinds::default_tag_filter(),
            quit: Keybinds::default_quit(),
        }
    }
}

impl Keybinds {
    fn default_move_up() -> KeyBinding {
        KeyBinding::key("up")
    }
    fn default_move_down() -> KeyBinding {
        KeyBinding::key("down")
    }
    fn default_expand() -> KeyBinding {
        KeyBinding::alt("e")
    }
    fn default_refresh() -> KeyBinding {
        KeyBinding::alt("r")
    }
    fn default_rename() -> KeyBinding {
        KeyBinding::alt("m")
    }
    fn default_new_workspace() -> KeyBinding {
        KeyBinding::alt("w")
    }
    fn default_delete() -> KeyBinding {
        KeyBinding::alt("d")
    }
    fn default_new_session() -> KeyBinding {
        KeyBinding::alt("n")
    }
    fn default_search() -> KeyBinding {
        KeyBinding::alt("/")
    }
    fn default_help() -> KeyBinding {
        KeyBinding::alt("k")
    }
    fn default_settings() -> KeyBinding {
        KeyBinding::alt("s")
    }
    fn default_theme() -> KeyBinding {
        KeyBinding::alt("t")
    }
    fn default_export() -> KeyBinding {
        KeyBinding::alt("x")
    }
    fn default_copy() -> KeyBinding {
        KeyBinding::alt("y")
    }
    fn default_preview() -> KeyBinding {
        KeyBinding::alt("v")
    }
    fn default_tag_filter() -> KeyBinding {
        KeyBinding::alt("f")
    }
    fn default_quit() -> KeyBinding {
        KeyBinding::alt("q")
    }
    /// Detect keybind conflicts. Returns a list of (action_a, action_b) pairs
    /// that share the same key binding.
    pub fn validate(&self) -> Vec<(&'static str, &'static str)> {
        let bindings: Vec<(&str, &KeyBinding)> = vec![
            ("move_up", &self.move_up),
            ("move_down", &self.move_down),
            ("expand", &self.expand),
            ("refresh", &self.refresh),
            ("rename", &self.rename),
            ("new_workspace", &self.new_workspace),
            ("delete", &self.delete),
            ("new_session", &self.new_session),
            ("search", &self.search),
            ("help", &self.help),
            ("settings", &self.settings),
            ("theme", &self.theme),
            ("export", &self.export),
            ("copy", &self.copy),
            ("preview", &self.preview),
            ("tag_filter", &self.tag_filter),
            ("quit", &self.quit),
        ];
        let mut conflicts = Vec::new();
        for i in 0..bindings.len() {
            for j in (i + 1)..bindings.len() {
                let (name_a, kb_a) = bindings[i];
                let (name_b, kb_b) = bindings[j];
                if kb_a.key == kb_b.key
                    && kb_a.ctrl == kb_b.ctrl
                    && kb_a.shift == kb_b.shift
                    && kb_a.alt == kb_b.alt
                {
                    conflicts.push((name_a, name_b));
                }
            }
        }
        conflicts
    }
    /// Return a formatted list of all keybindings for display.
    pub fn display_lines(&self) -> Vec<String> {
        vec![
            format!("  move_up:       {}", self.move_up.display()),
            format!("  move_down:     {}", self.move_down.display()),
            format!("  expand:        {}", self.expand.display()),
            format!("  refresh:       {}", self.refresh.display()),
            format!("  rename:        {}", self.rename.display()),
            format!("  new_workspace: {}", self.new_workspace.display()),
            format!("  delete:        {}", self.delete.display()),
            format!("  new_session:   {}", self.new_session.display()),
            format!("  search:        {}", self.search.display()),
            format!("  keybinds:      {}", self.help.display()),
            format!("  settings:      {}", self.settings.display()),
            format!("  theme:         {}", self.theme.display()),
            format!("  export:        {}", self.export.display()),
            format!("  copy:          {}", self.copy.display()),
            format!("  preview:       {}", self.preview.display()),
            format!("  tag_filter:    {}", self.tag_filter.display()),
            format!("  quit:          {}", self.quit.display()),
        ]
    }
    /// One-line hint string for the status bar.
    pub fn status_hint(&self) -> String {
        format!(
            "Enter:new {}:{} {}:{} o:open {}:{} Tab:toggle {}:quit",
            self.expand.display(),
            "exp",
            self.refresh.display(),
            "rfr",
            self.rename.display(),
            "ren",
            self.quit.display(),
        )
    }
    /// Key/action pairs for the help popup sidebar section (plain strings).
    pub fn help_sidebar_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            (
                "Move selection",
                format!("{}/{} ↑↓", self.move_up.display(), self.move_down.display()),
            ),
            ("New session / Resume / Switch", "Enter".into()),
            ("Expand / collapse", self.expand.display()),
            ("Refresh sessions", self.refresh.display()),
            ("Rename selected", self.rename.display()),
            ("New workspace", self.new_workspace.display()),
            ("Delete", self.delete.display()),
            ("New session (agent picker)", self.new_session.display()),
            ("Search sessions", self.search.display()),
            ("This help", self.help.display()),
            ("Quit", format!("{} / Esc", self.quit.display())),
        ]
    }
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
            theme: crate::theme::ThemeName::Dark,
            keybinds: Keybinds::default(),
            templates: Vec::new(),
            automations: Vec::new(),
            archive_days: None,
            remote_hosts: Vec::new(),
            plugins: Vec::new(),
            serve_port: None,
            serve_token: None,
            check_command: None,
            token_budget: None,
            chains: Vec::new(),
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
            preflight: Default::default(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ProjectConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.default_agent, Some("claude".to_string()));
        assert_eq!(parsed.check_command, Some("npm test".to_string()));
        assert_eq!(parsed.ignore_sessions, vec!["temp-"]);
        assert_eq!(parsed.env, vec![("NODE_ENV".into(), "development".into())]);
    }
}
