use std::path::PathBuf;

use serde::Deserialize;

use super::agent::Agent;

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
    pub const fn next(&self) -> SortMode {
        match self {
            SortMode::TimeDesc => SortMode::TimeAsc,
            SortMode::TimeAsc => SortMode::NameAsc,
            SortMode::NameAsc => SortMode::NameDesc,
            SortMode::NameDesc => SortMode::AgentGroup,
            SortMode::AgentGroup => SortMode::TimeDesc,
        }
    }

    pub const fn label(&self) -> &'static str {
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

impl PtySlot {
    /// Record a screen frame to disk (throttled to max every 200ms).
    /// Returns `true` if the screen content changed.
    pub fn record_screen_frame(&mut self) -> bool {
        use std::hash::{Hash, Hasher};
        if self.last_recording_at.elapsed() < std::time::Duration::from_millis(200) {
            return false;
        }
        let content = self.handle.screen_contents();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        let hash = hasher.finish();
        if hash == self.last_screen_hash {
            return false;
        }
        self.last_screen_hash = hash;
        self.last_recording_at = std::time::Instant::now();
        let rec_dir = crate::config::data_dir().join("recordings");
        let _ = std::fs::create_dir_all(&rec_dir);
        let id = self.info.session_id.as_deref().unwrap_or("unknown");
        let path = rec_dir.join(format!("{}.cast", &id[..id.len().min(16)]));
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let frame = serde_json::json!([ts, "o", content]);
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, format!("{frame}\n").as_bytes()));
        true
    }
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

/// Chat panel input mode — controls how keyboard/mouse events are dispatched.
///
/// - `Passthrough`: All keys forwarded directly to the PTY. Only `Alt+` sequences
///   are intercepted by amux. The agent program handles its own keybindings,
///   scrolling, and cursor. This is the default for normal interaction.
///
/// - `Amux`: amux intercepts Alt+Shift+B/F (scrollback), PageUp/Down, Home/End, `y`
///   (copy), Alt+Shift+F (search), etc. for its own features. Use when you want amux
///   scrollback/search and the agent is idle.
///
/// Toggle with `F12`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChatMode {
    /// All keys go straight to PTY.
    #[default]
    Passthrough,
    /// amux intercepts keys for scrollback, search, copy.
    Amux,
}

// InputMode consolidation evaluation — see docs/architecture-decisions/0001-inputmode-eval.md

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
    ScrollbackSearch,
}

#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

#[derive(Debug)]
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
