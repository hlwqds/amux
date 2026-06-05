use std::{
    collections::HashSet,
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
};

use crate::config::{data_dir, save_config_file, title_override_path};
use crate::discovery::{
    PreviewLine, SessionCache, discover_sessions, discover_sessions_cached, extract_branch_points,
    find_session_jsonl, preview_session_content,
};
use crate::pty::PtyState;
use crate::types::*;
use crate::util::*;
use anyhow::{Context, Result};
use crossterm::event::{Event, KeyEventKind};
use ratatui::{layout::Rect, widgets::ListState};

// ─── Sub-structures ──────────────────────────────────────────

#[derive(Clone)]
struct AppView {
    focus: Focus,
    input_mode: InputMode,
    chat_mode: ChatMode,
    status: String,
    prev_status: String,
    status_set_at: std::time::Instant,
    sort_mode: SortMode,
    agent_filter: Option<Agent>,
    tag_filter: Option<String>,
    search_query: Option<String>,
    selected_set: HashSet<usize>,
    last_chat_area: Rect,
    tab_bar_rect: Rect,
    theme_name: crate::theme::ThemeName,
    theme: crate::theme::Theme,
    keybinds: Keybinds,
    /// Whether any PTY screen content changed since last render.
    screen_changed: bool,
    /// Previous input_mode to detect mode transitions requiring re-render.
    prev_input_mode: InputMode,
    /// Active query for PTY scrollback search (Alt+Shift+F).
    scrollback_query: String,
    /// Match positions from scrollback search: (row, col, length).
    scrollback_matches: Vec<(u16, u16, usize)>,
    /// Currently highlighted match index in scrollback_matches.
    scrollback_match_idx: usize,
    /// Whether scrollback search uses regex mode.
    scrollback_regex: bool,
    /// Whether scrollback search is case-sensitive.
    scrollback_case_sensitive: bool,
    /// Active query for fuzzy filtering in picker popups (ThemeSelect, TemplateSelect, etc).
    picker_query: String,
    /// Sidebar/chat split percentage (20-50, default 30).
    split_ratio: u16,
    /// Whether user is currently dragging the split divider.
    dragging_split: bool,
    /// Related sessions for the active PTY (session_id, BM25 score), updated on tab switch.
    related_sessions: Vec<(String, f64)>,
}

#[derive(Clone, Default)]
struct PtyManager {
    ptys: Vec<PtySlot>,
    active_pty: Option<usize>,
    /// Monotonic counter for generating unique PTY IDs.
    pty_counter: u64,
    prev_states: Vec<PtyState>,
    /// Pending input steps queued for the active PTY.
    pending_inputs: Vec<PendingInput>,
}

#[derive(Clone, Default)]
struct SessionStore {
    workspaces: Vec<Workspace>,
    sessions: Vec<Session>,
    tree: Vec<TreeNode>,
    ws_session_map: Vec<Vec<usize>>,
    tree_state: ListState,
    /// Sessions older than archive_days, filtered from active display.
    archived_sessions: Vec<Session>,
    /// Whether to show archived sessions in the sidebar.
    show_archived: bool,
    archive_days: Option<u64>,
    /// Whether the virtual "Pinned" workspace is expanded.
    pinned_expanded: bool,
    /// Whether the virtual "Recent" workspace is expanded.
    recent_expanded: bool,
    /// Number of recent sessions (cached during rebuild_tree).
    recent_count: usize,
    /// Cache for incremental session discovery — maps file path to (mtime, Session).
    session_cache: SessionCache,
    /// Per-project configs keyed by workspace path, loaded from `.amux.json`.
    project_configs: std::collections::HashMap<PathBuf, ProjectConfig>,
    /// Modification times for `.amux.json` files — used to skip redundant reloads.
    project_config_mtimes: std::collections::HashMap<PathBuf, std::time::SystemTime>,
}

#[derive(Clone, Default)]
struct PopupState {
    preview_session_id: Option<String>,
    preview_lines: Vec<PreviewLine>,
    preview_show_summary: bool,
    /// Diff lines for the current diff view.
    diff_lines: Vec<crate::discovery::DiffLine>,
    /// Index of the first session selected for diff.
    diff_left_session: Option<usize>,
    /// Branch points for the current session being branched.
    branch_points: Vec<crate::discovery::BranchPoint>,
    conflict_warnings: Vec<String>,
    /// Files that would be affected by a rollback (for confirmation dialog).
    rollback_files: Vec<String>,
    /// The snapshot commit hash for the pending rollback.
    rollback_snapshot: Option<String>,
    /// Workspace path for the pending rollback.
    rollback_workspace: Option<PathBuf>,
    /// Active budget alert message, if budget exceeded.
    budget_alert: Option<String>,
    /// Whether the status bar is in flash-on state (toggles each render).
    budget_flash_on: bool,
    /// Whether the knowledge view is shown in SessionPreview.
    knowledge_view: bool,
    /// Pre-flight check results, if a popup is pending.
    preflight_result: Option<crate::preflight::PreflightResult>,
    /// Workspace path for the pending pre-flight check.
    preflight_workspace: Option<PathBuf>,
    /// Pending agent to spawn after pre-flight confirm.
    preflight_agent: Option<Agent>,
    /// Pending session name for pre-flight confirm.
    preflight_session_name: Option<String>,
    /// Scroll offset for the KeybindView popup.
    keybind_scroll: u16,
}

#[derive(Clone, Default)]
struct ChainState {
    /// Configured session chains from config.json.
    chains: Vec<crate::chain::SessionChain>,
    /// Currently executing chain, if any.
    active_chain: Option<crate::chain::ActiveChain>,
    /// List state for chain selection popup.
    chain_state: ListState,
}

impl Default for AppView {
    fn default() -> Self {
        Self {
            focus: Focus::default(),
            input_mode: InputMode::default(),
            chat_mode: ChatMode::default(),
            status: String::new(),
            prev_status: String::new(),
            status_set_at: std::time::Instant::now(),
            sort_mode: SortMode::default(),
            agent_filter: None,
            tag_filter: None,
            search_query: None,
            selected_set: HashSet::new(),
            last_chat_area: Rect::default(),
            tab_bar_rect: Rect::default(),
            theme_name: crate::theme::ThemeName::default(),
            theme: crate::theme::Theme::dark(),
            keybinds: Keybinds::default(),
            screen_changed: true,
            prev_input_mode: InputMode::default(),
            scrollback_query: String::new(),
            scrollback_matches: Vec::new(),
            scrollback_match_idx: 0,
            scrollback_regex: false,
            scrollback_case_sensitive: false,
            picker_query: String::new(),
            split_ratio: 30,
            dragging_split: false,
            related_sessions: Vec::new(),
        }
    }
}

struct App {
    view: AppView,
    ptys: PtyManager,
    /// Bottom terminal split — a shell PTY in the active session's cwd.
    terminal: Option<PtySlot>,
    sessions: SessionStore,
    popup: PopupState,
    chains: ChainState,
    // Remaining flat fields
    pending_delete: Option<TreeNode>,
    pending_batch_delete: bool,
    input_buffer: String,
    rename_target: Option<usize>,
    rename_workspace_target: Option<usize>,
    new_workspace_name: Option<String>,
    pending_session_name: Option<String>,
    available_agents: Vec<Agent>,
    agent_state: ListState,
    browse_dir: PathBuf,
    browse_entries: Vec<DirEntry>,
    browse_state: ListState,
    last_refresh: std::time::Instant,
    templates: Vec<SessionTemplate>,
    template_state: ListState,
    automations: Vec<InputAutomation>,
    automation_state: ListState,
    branch_state: ListState,
    remote_hosts: Vec<crate::types::RemoteHost>,
    remote_sessions: Vec<(String, String)>,
    plugins: Vec<crate::types::Plugin>,
    /// Last plugin output for display.
    plugin_output: Vec<String>,
    /// Scroll offset for plugin output view.
    plugin_scroll: usize,
    plugin_state: ListState,
    timeline_events: Vec<crate::discovery::TimelineEvent>,
    agent_recommendations: Vec<crate::discovery::AgentMetrics>,
    cross_search_results: Vec<crate::discovery::CrossSearchResult>,
    shared_ptys: std::sync::Arc<crate::server::SharedPtyMap>,
    server_handle: Option<tokio::task::JoinHandle<()>>,
    /// Override check command from config. Format: "command arg1 arg2"
    check_command: Option<String>,
    // --- Theme selection state ---
    /// Available themes for the theme selector (built-in + custom).
    theme_list: Vec<crate::theme::ThemeName>,
    /// List state for theme selection popup.
    theme_list_state: ListState,
    /// Active worktrees: (repo_path, branch_name) — cleaned up on exit.
    worktree_branches: Vec<(PathBuf, String)>,
    /// File paths detected in the active PTY screen, with optional line numbers.
    /// Token budget config (cached from Config).
    token_budget: Option<crate::budget::TokenBudget>,
    /// Last time detect_file_conflicts ran (throttled to 30s).
    last_conflict_check: std::time::Instant,
    last_budget_check: std::time::Instant,
    /// Last time process stats were collected from /proc (throttled to 30s).
    last_stats_check: std::time::Instant,
    /// BM25 search index for semantic-like session search.
    search_index: crate::search_engine::SearchIndex,
    /// Results from the last semantic search.
    search_results: Vec<(String, f64)>,
    /// List state for semantic search result selection.
    search_result_state: ratatui::widgets::ListState,
}

impl Default for App {
    fn default() -> Self {
        Self {
            view: AppView::default(),
            ptys: PtyManager::default(),
            terminal: None,
            sessions: SessionStore::default(),
            popup: PopupState::default(),
            chains: ChainState::default(),
            pending_delete: None,
            pending_batch_delete: false,
            input_buffer: String::new(),
            rename_target: None,
            rename_workspace_target: None,
            new_workspace_name: None,
            pending_session_name: None,
            available_agents: Vec::new(),
            agent_state: ListState::default(),
            browse_dir: PathBuf::new(),
            browse_entries: Vec::new(),
            browse_state: ListState::default(),
            last_refresh: std::time::Instant::now(),
            templates: Vec::new(),
            template_state: ListState::default(),
            automations: Vec::new(),
            automation_state: ListState::default(),
            branch_state: ListState::default(),
            remote_hosts: Vec::new(),
            remote_sessions: Vec::new(),
            plugins: Vec::new(),
            plugin_output: Vec::new(),
            plugin_scroll: 0,
            plugin_state: ListState::default(),
            timeline_events: Vec::new(),
            agent_recommendations: Vec::new(),
            cross_search_results: Vec::new(),
            shared_ptys: std::sync::Arc::new(crate::server::SharedPtyMap::new()),
            server_handle: None,
            check_command: None,
            theme_list: Vec::new(),
            theme_list_state: ListState::default(),
            worktree_branches: Vec::new(),
            token_budget: None,
            last_conflict_check: std::time::Instant::now(),
            last_budget_check: std::time::Instant::now(),
            last_stats_check: std::time::Instant::now(),
            search_index: crate::search_engine::SearchIndex::new(),
            search_results: Vec::new(),
            search_result_state: ListState::default(),
        }
    }
}

mod browse;
mod chain_handler;
mod handler;
mod handler_search;
mod handler_select;
mod session;
mod ui;
mod ui_popup;
mod session_ops;
impl App {
    fn new(
        shared_ptys: std::sync::Arc<crate::server::SharedPtyMap>,
    ) -> Self {
        let mut config = crate::config::load_config().unwrap_or_else(|_| Config {
            workspaces: Vec::new(),
            ..Default::default()
        });

        if config.workspaces.is_empty() {
            let _ = save_config_file(&config);
        }
        let check_command = config.check_command.take();
        let token_budget = config.token_budget.take();

        for ws in &mut config.workspaces {
            ws.expanded = true;
        }

        let sessions_list = discover_sessions(&config.workspaces);
        let mut app = Self {
            view: AppView {
                focus: Focus::Sidebar,
                input_mode: InputMode::None,
                chat_mode: ChatMode::default(),
                status: Default::default(),
                prev_status: String::new(),
                status_set_at: std::time::Instant::now(),
                sort_mode: SortMode::default(),
                agent_filter: None,
                tag_filter: None,
                search_query: None,
                selected_set: HashSet::new(),
                last_chat_area: Rect::default(),
                tab_bar_rect: Rect::default(),
                theme_name: config.theme.clone(),
                theme: config.theme.theme(),
                keybinds: config.keybinds,
                screen_changed: true,
                prev_input_mode: InputMode::None,
                scrollback_query: String::new(),
                scrollback_matches: Vec::new(),
                scrollback_match_idx: 0,
                scrollback_regex: false,
                scrollback_case_sensitive: false,
                picker_query: String::new(),
                split_ratio: 30,
                dragging_split: false,
                related_sessions: Vec::new(),
            },
            ptys: PtyManager::default(),
            terminal: None,
            sessions: SessionStore {
                workspaces: config.workspaces,
                sessions: sessions_list,
                tree: Vec::new(),
                ws_session_map: Vec::new(),
                tree_state: ListState::default(),
                archived_sessions: Vec::new(),
                show_archived: false,
                archive_days: config.archive_days,
                pinned_expanded: false,
                recent_expanded: false,
                recent_count: 0,
                session_cache: SessionCache::new(),
                project_configs: std::collections::HashMap::new(),
                project_config_mtimes: std::collections::HashMap::new(),
            },
            popup: PopupState::default(),
            chains: ChainState {
                chains: config.chains,
                active_chain: None,
                chain_state: ListState::default(),
            },
            pending_delete: None,
            pending_batch_delete: false,
            input_buffer: String::new(),
            rename_target: None,
            rename_workspace_target: None,
            new_workspace_name: None,
            pending_session_name: None,
            available_agents: detect_agents(),
            agent_state: ListState::default(),
            browse_dir: PathBuf::new(),
            browse_entries: Vec::new(),
            browse_state: ListState::default(),
            last_refresh: std::time::Instant::now(),
            templates: config.templates,
            template_state: ListState::default(),
            automations: config.automations,
            automation_state: ListState::default(),
            branch_state: ListState::default(),
            remote_hosts: config.remote_hosts,
            remote_sessions: Vec::new(),
            plugins: config.plugins,
            plugin_output: Vec::new(),
            plugin_scroll: 0,
            plugin_state: ListState::default(),
            timeline_events: Vec::new(),
            agent_recommendations: Vec::new(),
            cross_search_results: Vec::new(),
            shared_ptys,
            server_handle: None,
            check_command,
            theme_list: Vec::new(),
            theme_list_state: ListState::default(),
            worktree_branches: Vec::new(),
            token_budget,
            last_conflict_check: std::time::Instant::now(),
            last_budget_check: std::time::Instant::now(),
            last_stats_check: std::time::Instant::now(),
            search_index: crate::search_engine::SearchIndex::new(),
            search_results: Vec::new(),
            search_result_state: ListState::default(),
        };
        app.rebuild_tree();
        if !app.sessions.tree.is_empty() {
            app.sessions.tree_state.select(Some(0));
        }
        // P1: Warn about keybind conflicts at startup
        let conflicts = app.view.keybinds.validate();
        if !conflicts.is_empty() {
            for (a, b) in &conflicts {
                eprintln!("warning: keybind conflict: {} <-> {}", a, b);
            }
            app.view.status = format!(
                "⚠ {} keybind conflict(s) detected — check logs",
                conflicts.len()
            );
        }
        // Quick environment diagnostics on first launch
        if let Some(warning) = crate::doctor::run_quick_doctor() {
            app.view.status = warning;
        } else if app.view.status.is_empty() {
            app.view.status = app.view.keybinds.status_hint();
        }
        app
    }

    fn cycle_sort_mode(&mut self) {
        self.view.sort_mode = self.view.sort_mode.next();
        self.rebuild_tree();
        self.view.status = format!("Sort: {}", self.view.sort_mode.label());
    }

    fn sort_session_indices(&self, indices: &mut [usize]) {
        let ss = &self.sessions.sessions;
        // Pinned sessions always come first, regardless of sort mode
        let pin_cmp = |a: usize, b: usize| ss[b].pinned.cmp(&ss[a].pinned);
        match self.view.sort_mode {
            SortMode::TimeDesc => indices.sort_by(|&a, &b| {
                pin_cmp(a, b).then_with(|| ss[b].last_active.cmp(&ss[a].last_active))
            }),
            SortMode::TimeAsc => indices.sort_by(|&a, &b| {
                pin_cmp(a, b).then_with(|| ss[a].last_active.cmp(&ss[b].last_active))
            }),
            SortMode::NameAsc => indices.sort_by(|&a, &b| {
                pin_cmp(a, b)
                    .then_with(|| ss[a].title.to_lowercase().cmp(&ss[b].title.to_lowercase()))
            }),
            SortMode::NameDesc => indices.sort_by(|&a, &b| {
                pin_cmp(a, b)
                    .then_with(|| ss[b].title.to_lowercase().cmp(&ss[a].title.to_lowercase()))
            }),
            SortMode::AgentGroup => indices.sort_by(|&a, &b| {
                pin_cmp(a, b).then_with(|| {
                    ss[a]
                        .agent
                        .cmp(&ss[b].agent)
                        .then_with(|| ss[b].last_active.cmp(&ss[a].last_active))
                })
            }),
        }
    }

    fn append_agent_grouped(
        sessions: &[Session],
        indices: &[usize],
        wi: usize,
        tree: &mut Vec<TreeNode>,
    ) {
        let agent_order = [Agent::Claude, Agent::Codex, Agent::Omp];
        for agent in agent_order {
            let group: Vec<usize> = indices
                .iter()
                .copied()
                .filter(|&si| sessions[si].agent == agent)
                .collect();
            if group.is_empty() {
                continue;
            }
            tree.push(TreeNode::AgentHeader(agent));
            for &si in &group {
                tree.push(TreeNode::Session(wi, si));
            }
        }
    }

    fn poll_states(&mut self) {
        // Track when status string changes for auto-expire in status bar
        // Track when status string changes for auto-expire in status bar
        if self.view.status != self.view.prev_status {
            self.view.prev_status = self.view.status.clone();
            self.view.status_set_at = std::time::Instant::now();
        }
        let mut notification = None;
        let pty_count = self.ptys.ptys.len();
        // Deferred chain step to execute after the PTY loop (borrow checker)
        let mut chain_step: Option<crate::chain::ActiveChain> = None;
        let mut chain_completed = false;
        let mut any_screen_changed = false;
        // Pre-extract chain data so we don't borrow self inside iter_mut
        let pre_chain = self.chains.active_chain.clone();
        let pre_session_map: std::collections::HashMap<String, PathBuf> =
            if !self.ptys.ptys.is_empty() && self.chains.active_chain.is_some() {
                self.sessions
                    .sessions
                    .iter()
                    .filter_map(|s| {
                        crate::discovery::find_session_jsonl(s).map(|p| (s.id.clone(), p))
                    })
                    .collect()
            } else {
                std::collections::HashMap::new()
            };
        for (i, slot) in self.ptys.ptys.iter_mut().enumerate() {
            let state = slot.handle.state();
            if state == PtyState::Running {
                slot.info.completed = false;
                // Record screen frame (throttled to max every 200ms)
                if slot.last_recording_at.elapsed() >= std::time::Duration::from_millis(200) {
                    use std::hash::{Hash, Hasher};
                    let content = slot.handle.screen_contents();
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    content.hash(&mut hasher);
                    let hash = hasher.finish();
                    if hash != slot.last_screen_hash {
                        slot.last_screen_hash = hash;
                        any_screen_changed = true;
                        let rec_dir = crate::config::data_dir().join("recordings");
                        let _ = std::fs::create_dir_all(&rec_dir);
                        let id = slot.info.session_id.as_deref().unwrap_or("unknown");
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
                            .and_then(|mut f| {
                                std::io::Write::write_all(&mut f, format!("{}\n", frame).as_bytes())
                            });
                    }
                    slot.last_recording_at = std::time::Instant::now();
                }
            } else if !slot.info.completed && state == PtyState::Completed {
                slot.info.completed = true;
                any_screen_changed = true;

                if self.ptys.prev_states.get(i) == Some(&PtyState::Running) {
                    notification = Some(format!(
                        "✔ {} completed ({}/{})",
                        slot.info.title,
                        i + 1,
                        pty_count
                    ));
                    // Save recording metadata
                    let rec_dir = crate::config::data_dir().join("recordings");
                    let id = slot.info.session_id.as_deref().unwrap_or("unknown");
                    let path = rec_dir.join(format!("{}.cast", &id[..id.len().min(16)]));
                    if path.exists() {
                        let header = serde_json::json!({
                            "version": 2, "width": 80, "height": 24,
                            "timestamp": slot.info.started_at,
                            "title": slot.info.title,
                            "env": { "TERM": "xterm-256color" }
                        });
                        let header_path =
                            rec_dir.join(format!("{}.meta.json", &id[..id.len().min(16)]));
                        let _ = std::fs::write(
                            &header_path,
                            serde_json::to_string_pretty(&header).unwrap_or_default(),
                        );
                    }
                    // Desktop notification deferred to after the loop
                    // Record git state + diff summary (merged git calls)
                    {
                        let ws = &slot.info.workspace_path;
                        let branch = git_cmd(ws, &["rev-parse", "--abbrev-ref", "HEAD"]).ok();
                        let commit = git_cmd(ws, &["rev-parse", "--short", "HEAD"]).ok();
                        // One --stat call for both git_info.diff_stat and diff_summary.summary_line
                        let stat_output = git_cmd(ws, &["diff", "--stat"]).ok();
                        let diff_stat = stat_output.as_ref().and_then(|s| {
                            if s.is_empty() {
                                None
                            } else {
                                s.lines().last().map(|l| l.to_string())
                            }
                        });
                        let summary_line = stat_output.and_then(|s| {
                            if s.is_empty() {
                                None
                            } else {
                                s.lines().last().map(|l| l.to_string())
                            }
                        });
                        // One --numstat call for both files_changed and insertions/deletions
                        let numstat_output = git_cmd(ws, &["diff", "--numstat"]).ok();
                        let (files, ins, del) = numstat_output
                            .map(|s| {
                                let (files, total_ins, total_del) = s
                                    .lines()
                                    .filter(|l| !l.is_empty())
                                    .fold((Vec::new(), 0u32, 0u32), |(mut f, a, b), l| {
                                        let parts: Vec<&str> = l.split('\t').collect();
                                        if parts.len() >= 3 {
                                            f.push(parts[2].to_string());
                                            (
                                                f,
                                                a + parts[0].parse::<u32>().unwrap_or(0),
                                                b + parts[1].parse::<u32>().unwrap_or(0),
                                            )
                                        } else if parts.len() >= 2 {
                                            (
                                                f,
                                                a + parts[0].parse::<u32>().unwrap_or(0),
                                                b + parts[1].parse::<u32>().unwrap_or(0),
                                            )
                                        } else {
                                            (f, a, b)
                                        }
                                    });
                                (files, total_ins, total_del)
                            })
                            .unwrap_or_default();
                        slot.info.git_info = GitInfo {
                            branch,
                            commit,
                            diff_stat,
                        };
                        slot.info.diff_summary = DiffSummary {
                            files_changed: files,
                            insertions: ins,
                            deletions: del,
                            summary_line,
                        };
                    }
                    // Save snapshot_commit to session meta for rollback support
                    if let Some(ref session_id) = slot.info.session_id
                        && let Some(ref snapshot) = slot.info.snapshot_commit
                    {
                        let _ = crate::config::save_snapshot_meta(session_id, snapshot);
                    }
                    // Auto-generate session summary (saved to file, no popup)
                    if let Some(ref session_id) = slot.info.session_id
                        && let Some(session) =
                            self.sessions.sessions.iter().find(|s| &s.id == session_id)
                        && let Some(summary) = crate::discovery::generate_session_summary(session)
                    {
                        let summary_dir = crate::config::data_dir().join("summaries");
                        let _ = std::fs::create_dir_all(&summary_dir);
                        let short_id = &session_id[..session_id.len().min(16)];
                        let path = summary_dir.join(format!("{}.md", short_id));
                        let _ = std::fs::write(&path, &summary);
                    }
                    // Merge session knowledge into workspace knowledge base
                    {
                        let ws = &slot.info.workspace_path;
                        let summary_text = if let Some(ref session_id) = slot.info.session_id {
                            let short_id = &session_id[..session_id.len().min(16)];
                            let summary_path = crate::config::data_dir()
                                .join("summaries")
                                .join(format!("{}.md", short_id));
                            std::fs::read_to_string(&summary_path).unwrap_or_default()
                        } else {
                            String::new()
                        };
                        if !summary_text.is_empty() {
                            let mut kb = crate::knowledge::load_knowledge(ws);
                            crate::knowledge::merge_from_session(&mut kb, &summary_text);
                            let _ = crate::knowledge::save_knowledge(ws, &kb);
                        }
                    }
                    // Execute on_complete hook plugins
                    {
                        let session_id = slot.info.session_id.clone().unwrap_or_default();
                        let title = slot.info.title.clone();
                        for plugin in &self.plugins {
                            if plugin.hooks.iter().any(|h| h == "on_complete") {
                                let mut cmd = plugin
                                    .command
                                    .replace(
                                        "{workspace}",
                                        &slot.info.workspace_path.to_string_lossy(),
                                    )
                                    .replace("{session_id}", &session_id)
                                    .replace("{title}", &title);
                                cmd.push_str(&format!(
                                    " --event on_complete --session_id {}",
                                    session_id
                                ));
                                let _ = std::process::Command::new("sh")
                                    .arg("-c")
                                    .arg(&cmd)
                                    .output();
                            }
                        }
                    }
                    // Chain continuation: pre-extracted data (can't borrow self inside iter_mut)
                    if let Some(ref pc) = pre_chain
                        && slot.info.completed
                    {
                        if pc.current_step + 1 < pc.total_steps {
                            let prev_output = slot
                                .info
                                .session_id
                                .as_ref()
                                .and_then(|sid| pre_session_map.get(sid.as_str()))
                                .and_then(|path| crate::discovery::extract_session_output(path));
                            let mut updated = pc.clone();
                            updated.prev_output = prev_output;
                            updated.current_step += 1;
                            chain_step = Some(updated);
                        } else {
                            chain_completed = true;
                        }
                    }
                    {
                        let project_type = slot.info.project_type;
                        slot.info.check_status = CheckStatus::Running;
                        let ws = slot.info.workspace_path.clone();
                        let idx = i;
                        // Global override first, then project config, then auto-detect
                        let check_override = self.check_command.clone().or_else(|| {
                            self.sessions
                                .project_configs
                                .get(&ws)
                                .and_then(|pc| pc.check_command.clone())
                        });
                        std::thread::spawn(move || {
                            let result = if let Some(ref cmd_str) = check_override {
                                // User-configured override command
                                let parts: Vec<&str> = cmd_str.split_whitespace().collect();
                                if parts.is_empty() {
                                    CheckStatus::Passed
                                } else {
                                    let (prog, args) = (parts[0], &parts[1..]);
                                    let out = std::process::Command::new(prog)
                                        .args(args)
                                        .current_dir(&ws)
                                        .output();
                                    if out.as_ref().map(|o| o.status.success()).unwrap_or(false) {
                                        CheckStatus::Passed
                                    } else {
                                        CheckStatus::Failed(cmd_str.clone())
                                    }
                                }
                            } else {
                                let commands = project_type.check_commands();
                                if commands.is_empty() {
                                    // Unknown project type — skip check
                                    CheckStatus::Passed
                                } else {
                                    let mut errs = Vec::new();
                                    for (prog, args) in &commands {
                                        let out = std::process::Command::new(*prog)
                                            .args(args)
                                            .current_dir(&ws)
                                            .output();
                                        if !out
                                            .as_ref()
                                            .map(|o| o.status.success())
                                            .unwrap_or(false)
                                        {
                                            errs.push(format!(
                                                "{} {} failed",
                                                prog,
                                                args.join(" ")
                                            ));
                                        }
                                    }
                                    if errs.is_empty() {
                                        CheckStatus::Passed
                                    } else {
                                        CheckStatus::Failed(errs.join(", "))
                                    }
                                }
                            };
                            // Store result via a marker file for the main thread to pick up
                            let marker =
                                crate::config::data_dir().join(format!(".check-result-{}", idx));
                            let _ = std::fs::write(
                                &marker,
                                serde_json::to_string(&result).unwrap_or_default(),
                            );
                        });
                    }
                }
            }
        }

        // Execute deferred chain step (borrow checker requires this outside iter_mut)
        self.execute_chain_step(chain_step, chain_completed);

        // Poll check results from background threads
        for i in 0..self.ptys.ptys.len() {
            let marker = crate::config::data_dir().join(format!(".check-result-{}", i));
            if marker.exists() {
                if let Ok(content) = std::fs::read_to_string(&marker)
                    && let Ok(status) = serde_json::from_str::<CheckStatus>(&content)
                {
                    self.ptys.ptys[i].info.check_status = status;
                }
                let _ = std::fs::remove_file(&marker);
            }
        }

        let before = self.ptys.ptys.len();
        // Unregister dead PTYs from shared state before retaining only live ones.
        for slot in &self.ptys.ptys {
            if !slot.handle.is_alive() {
                self.unregister_pty(&slot.id);
            }
        }
        self.ptys.ptys.retain(|slot| slot.handle.is_alive());
        if self.ptys.ptys.len() != before {
            if let Some(cur) = self.ptys.active_pty
                && cur >= self.ptys.ptys.len()
            {
                self.ptys.active_pty = if self.ptys.ptys.is_empty() {
                    None
                } else {
                    Some(self.ptys.ptys.len() - 1)
                };
            }
            self.rebuild_tree();
        }

        if let Some(ref msg) = notification {
            self.view.status = msg.clone();
            self.send_desktop_notification("amux", msg);
        }

        self.ptys.prev_states = self.ptys.ptys.iter().map(|s| s.handle.state()).collect();
        // Auto-close terminal split when shell exits
        // Auto-close terminal split when shell exits
        if let Some(term) = &self.terminal
            && term.handle.state() == PtyState::Completed
        {
            self.terminal = None;
            self.view.screen_changed = true;
        }
        // Detect terminal split screen changes
        if let Some(term) = &mut self.terminal
            && term.handle.state() == PtyState::Running
        {
            use std::hash::{Hash, Hasher};
            let content = term.handle.screen_contents();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            content.hash(&mut hasher);
            let hash = hasher.finish();
            if hash != term.last_screen_hash {
                term.last_screen_hash = hash;
                any_screen_changed = true;
            }
        }
        if any_screen_changed {
            self.view.screen_changed = true;
        }
    }

    fn send_desktop_notification(&self, title: &str, body: &str) {
        // Fire-and-forget desktop notification via notify-send
        let _ = std::process::Command::new("notify-send")
            .arg("--icon=utilities-terminal")
            .arg(title)
            .arg(body)
            .spawn();
    }
    fn pty_display_state(&self, pi: usize) -> PtyState {
        if let Some(slot) = self.ptys.ptys.get(pi) {
            slot.handle.state()
        } else {
            PtyState::Running
        }
    }
    fn pty_index_for_session(&self, session_id: &str) -> Option<usize> {
        self.ptys
            .ptys
            .iter()
            .position(|s| s.info.session_id.as_deref() == Some(session_id))
    }
    /// Generate a unique PTY ID and advance the counter.
    fn next_pty_id(&mut self) -> String {
        self.ptys.pty_counter += 1;
        format!("pty-{}", self.ptys.pty_counter)
    }
    /// Register a PTY handle in the shared state so the HTTP server can access it.
    fn register_pty(&self, id: &str, slot: &PtySlot) {
        let entry = crate::server::RegisteredPty {
            handle: std::sync::Arc::new(slot.handle.clone()),
            title: slot.info.title.clone(),
            agent: slot.info.agent,
            session_id: slot.info.session_id.clone(),
            process_stats: None,
        };
        self.shared_ptys.insert(id.to_string(), entry);
    }
    /// Unregister a PTY handle from the shared state.
    fn unregister_pty(&self, id: &str) {
        self.shared_ptys.remove(id);
    }
    /// Sync process stats from PtySlots to the shared PTY state for HTTP API.
    fn sync_pty_stats(&self) {
        for slot in &self.ptys.ptys {
            if let Some(mut entry) = self.shared_ptys.get_mut(&slot.id) {
                entry.process_stats = slot.process_stats.clone();
            }
        }
    }
    fn selected_node(&self) -> Option<&TreeNode> {
        self.sessions
            .tree_state
            .selected()
            .and_then(|i| self.sessions.tree.get(i))
    }
    fn workspace_cwd(&self, wi: usize) -> PathBuf {
        match &self.sessions.workspaces[wi].path {
            Some(p) => p.clone(),
            None => {
                let dir = data_dir()
                    .join("workspaces")
                    .join(&self.sessions.workspaces[wi].id);
                let _ = fs::create_dir_all(&dir);
                dir
            }
        }
    }
    fn ws_matches_path(&self, wi: usize, path: &Path) -> bool {
        match &self.sessions.workspaces[wi].path {
            Some(p) => p == path,
            None => path == self.workspace_cwd(wi),
        }
    }
    /// Get the workspace path for a tree node, if applicable.
    fn node_workspace_path(&self, node: &TreeNode) -> Option<PathBuf> {
        match node {
            TreeNode::Workspace(wi) => Some(self.workspace_cwd(*wi)),
            TreeNode::Session(wi, _) | TreeNode::ArchivedSession(wi, _) => {
                Some(self.workspace_cwd(*wi))
            }
            _ => None,
        }
    }
    /// Get project env vars for a workspace path from the loaded project config.
    fn project_env(&self, path: &Path) -> Vec<(String, String)> {
        self.sessions
            .project_configs
            .get(path)
            .map(|pc| pc.env.clone())
            .unwrap_or_default()
    }
    /// Get the default agent for a workspace path, if configured in .amux.json.
    fn default_agent_for_workspace(&self, path: &Path) -> Option<Agent> {
        let agent_name = self
            .sessions
            .project_configs
            .get(path)?
            .default_agent
            .as_ref()?;
        Agent::from_label(agent_name)
    }
}

use std::sync::LazyLock;
static URL_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"https?://[^\s)'"<>]+"#).expect("URL regex is a valid compile-time constant")
});

/// Extract a URL from a line of text containing the given column position.
fn extract_url_from_line(line: &str, click_col: usize) -> Option<String> {
    for mat in URL_RE.find_iter(line) {
        if mat.start() <= click_col && click_col <= mat.end() {
            let url = mat.as_str().trim_end_matches([',', '.', ';', ':']);
            return Some(url.to_string());
        }
    }
    None
}

/// Returns true if any of the `haystacks` fuzzy-matches `query` using code_fuzzy_match.
fn session_fuzzy_score(title: &str, short_id: &str, query: &str) -> bool {
    let check = |text: &str| -> bool {
        code_fuzzy_match::fuzzy_match(text, query).is_some_and(|score| score > 0)
    };
    check(title) || check(short_id)
}

/// Run a git command in the given directory and return stdout on success,
/// or a human-readable error message on failure.
pub(crate) fn git_cmd(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("git not available: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.contains("not a git repository") {
            Err("Not a git repo. Run 'git init' or check workspace path.".into())
        } else if stderr.contains("detached HEAD") {
            Err("Detached HEAD state. Checkout a branch first.".into())
        } else if !stderr.is_empty() {
            Err(stderr)
        } else {
            Err(format!(
                "git {} failed (exit {})",
                args.join(" "),
                output.status
            ))
        }
    }
}

// ─── Main ─────────────────────────────────────────────────

pub fn run(serve: bool) -> anyhow::Result<()> {
    let agents = detect_agents();
    if agents.is_empty() {
        anyhow::bail!("No agent CLI found. Install Claude Code, Codex, or OMP.");
    }
    crate::config::ensure_data_dir().context("failed to create data directory")?;
    let shared_ptys: std::sync::Arc<crate::server::SharedPtyMap> =
        std::sync::Arc::new(crate::server::SharedPtyMap::new());
    let mut app = App::new(shared_ptys.clone());
    // Only start embedded server with --web flag
    if serve {
        let rt = tokio::runtime::Runtime::new()?;
        let _guard = rt.enter();
        let config = crate::config::load_config().unwrap_or_else(|_| Config {
            workspaces: Vec::new(),
            ..Default::default()
        });
        let serve_port = config.serve_port.unwrap_or(8080);
        let serve_token = config.serve_token.clone().unwrap_or_default();
        let actual_port = match rt.block_on(crate::server::run_server_with_state(
            serve_port,
            serve_token,
            shared_ptys.clone(),
        )) {
            Ok((port, handle)) => {
                app.server_handle = Some(handle);
                port
            }
            Err(_) => {
                eprintln!("amux: port {} in use, trying random port", serve_port);
                match rt.block_on(crate::server::run_server_with_state(
                    0,
                    config.serve_token.clone().unwrap_or_default(),
                    shared_ptys.clone(),
                )) {
                    Ok((port, handle)) => {
                        app.server_handle = Some(handle);
                        port
                    }
                    Err(e) => {
                        eprintln!("amux: server disabled ({})", e);
                        0u16
                    }
                }
            }
        };
        if actual_port > 0 {
            app.view.status = format!(
                "{} [web: http://localhost:{}]",
                app.view.status, actual_port
            );
        }
        // Keep runtime alive for the server's lifetime — leak is intentional,
        // the process is about to enter the TUI event loop.
        std::mem::forget(rt);
    }
    if !std::io::stdout().is_terminal() {
        let sessions = discover_sessions(&app.sessions.workspaces);
        for (wi, ws) in app.sessions.workspaces.iter().enumerate() {
            println!(
                "{} {}",
                ws.name,
                ws.path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "virtual".into())
            );
            for s in sessions
                .iter()
                .filter(|s| app.ws_matches_path(wi, &s.workspace_path))
            {
                println!(
                    "  [{}] {} - {}",
                    &s.id[..8],
                    relative_time(s.last_active),
                    s.title
                );
            }
        }
        return Ok(());
    }
    let mut terminal = init_terminal()?;
    // Watch agent session directories for automatic refresh
    let watcher = crate::watch::SessionWatcher::new();
    let result = loop {
        // Yield to let PTY reader threads process any pending echo output
        // before rendering.  This ensures cursor position updates from agent
        // programs are reflected in the alacritty grid when we render.
        if app.view.screen_changed && !app.ptys.ptys.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        app.poll_states();
        app.flush_pending_inputs();

        // Detect mode transitions that require re-render
        let mode_changed = app.view.input_mode != app.view.prev_input_mode;
        if mode_changed {
            app.view.prev_input_mode = app.view.input_mode;
            // Clear fuzzy picker query when entering any picker mode
            if matches!(
                app.view.input_mode,
                InputMode::ThemeSelect
                    | InputMode::TemplateSelect
                    | InputMode::SelectAgent
                    | InputMode::BranchSelect
                    | InputMode::AutomationSelect
                    | InputMode::PluginList
            ) {
                app.view.picker_query.clear();
            }
        }
        // Only re-render when something actually changed
        let needs_render = app.view.screen_changed || mode_changed || !app.ptys.ptys.is_empty();
        if needs_render {
            terminal.draw(|frame| app.render(frame))?;
            app.view.screen_changed = false;
            // Move cursor to PTY input position so IME candidate window
            // follows the actual typing position. Required after every
            // frame because ratatui's full-screen redraw displaces the
            // real cursor.
            if app.view.focus == Focus::Chat
                && app.view.input_mode == InputMode::None
                && let Some(idx) = app.ptys.active_pty
                && let Some(slot) = app.ptys.ptys.get(idx)
            {
                let term = slot.handle.term();
                let guard = term.lock();
                let grid = guard.grid();
                let cursor_point = grid.cursor.point;
                let cursor_col = cursor_point.column.0 as u16;
                let cursor_row = (grid.display_offset() as i32 + cursor_point.line.0) as u16;
                let cursor_visible = guard.mode().contains(
                    alacritty_terminal::term::TermMode::SHOW_CURSOR,
                );
                let cursor_style = guard.cursor_style();
                drop(guard);
                let rect = app.view.last_chat_area;
                if cursor_row < rect.height && cursor_col < rect.width {
                    // Only forward cursor shape when the PTY program has
                    // explicitly changed it from the default (Block, not
                    // blinking).  This avoids overriding the user's terminal
                    // cursor preference (e.g. beam) when agents don't set it.
                    let default_shape = alacritty_terminal::vte::ansi::CursorShape::Block;
                    let is_explicit = cursor_style.shape != default_shape || cursor_style.blinking;
                    if is_explicit {
                        use crossterm::cursor::SetCursorStyle;
                        let shape = match cursor_style.shape {
                            alacritty_terminal::vte::ansi::CursorShape::Block => {
                                if cursor_style.blinking {
                                    SetCursorStyle::BlinkingBlock
                                } else {
                                    SetCursorStyle::SteadyBlock
                                }
                            }
                            alacritty_terminal::vte::ansi::CursorShape::Underline => {
                                if cursor_style.blinking {
                                    SetCursorStyle::BlinkingUnderScore
                                } else {
                                    SetCursorStyle::SteadyUnderScore
                                }
                            }
                            alacritty_terminal::vte::ansi::CursorShape::Beam => {
                                if cursor_style.blinking {
                                    SetCursorStyle::BlinkingBar
                                } else {
                                    SetCursorStyle::SteadyBar
                                }
                            }
                            alacritty_terminal::vte::ansi::CursorShape::HollowBlock => {
                                SetCursorStyle::SteadyBlock
                            }
                            alacritty_terminal::vte::ansi::CursorShape::Hidden => {
                                SetCursorStyle::SteadyBlock
                            }
                        };
                        let _ = crossterm::execute!(
                            std::io::stdout(),
                            shape,
                            crossterm::cursor::MoveTo(
                                rect.x + cursor_col,
                                rect.y + cursor_row + 2,
                            ),
                            crossterm::cursor::Show,
                        );
                    } else {
                        let _ = crossterm::execute!(
                            std::io::stdout(),
                            crossterm::cursor::MoveTo(
                                rect.x + cursor_col,
                                rect.y + cursor_row + 2,
                            ),
                            crossterm::cursor::Show,
                        );
                    }
                }
                // Hide cursor when terminal program hides it (e.g. alternate-screen apps).
                if !cursor_visible {
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Hide);
                }
            }
        }

        // Auto-refresh: either timer-based (5s) or file-system-event-driven
        let timer_due = app.last_refresh.elapsed() > std::time::Duration::from_secs(5);
        let fs_changed = watcher.poll();
        if !app.ptys.ptys.is_empty() && (timer_due || fs_changed) {
            app.refresh_sessions();
            app.update_related_sessions();
            app.last_refresh = std::time::Instant::now();
        }

        // Adaptive poll: shorter when PTYs are active, longer when idle
        let poll_ms = if app.ptys.ptys.is_empty() { 100 } else { 50 };
        if crossterm::event::poll(std::time::Duration::from_millis(poll_ms))? {
            match crossterm::event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match app.handle_key(key)? {
                    Action::Continue => {}
                    Action::Quit => break Ok(()),
                },
                Event::Paste(text) => {
                    app.handle_paste(&text)?;
                }

                Event::Mouse(mouse) => {
                    // Handle split divider drag first — consumes Down/Drag/Up near boundary
                    if app.handle_split_drag(mouse.kind, mouse.column) {
                        // consumed
                    } else {
                        match mouse.kind {
                            crossterm::event::MouseEventKind::Down(
                                crossterm::event::MouseButton::Left,
                            ) => {
                                if mouse
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL)
                                {
                                    app.ctrl_click_open(mouse.column, mouse.row);
                                } else {
                                    app.handle_mouse_click(mouse.column, mouse.row);
                                }
                            }
                            crossterm::event::MouseEventKind::Down(
                                crossterm::event::MouseButton::Right,
                            )
                            | crossterm::event::MouseEventKind::Down(
                                crossterm::event::MouseButton::Middle,
                            ) => {
                                app.handle_tab_close_click(mouse.column, mouse.row);
                            }
                            crossterm::event::MouseEventKind::ScrollUp => {
                                if app.view.focus == Focus::Chat
                                    && let Some(idx) = app.ptys.active_pty
                                    && let Some(slot) = app.ptys.ptys.get(idx)
                                {
                                    slot.handle.scroll_page_up(3);
                                }
                            }
                            crossterm::event::MouseEventKind::ScrollDown => {
                                if app.view.focus == Focus::Chat
                                    && let Some(idx) = app.ptys.active_pty
                                    && let Some(slot) = app.ptys.ptys.get(idx)
                                {
                                    slot.handle.scroll_page_down(3);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            // Any input event should trigger a re-render
            app.view.screen_changed = true;
        }
    };

    // Clean up any worktrees created during the session
    app.cleanup_worktrees();

    for slot in &app.ptys.ptys {
        app.unregister_pty(&slot.id);
    }
    app.ptys.ptys.clear();
    // Abort the background server
    if let Some(handle) = app.server_handle.take() {
        handle.abort();
    }
    restore_terminal(&mut terminal)?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal App with given workspaces + sessions for filter testing.
    /// Bypasses config loading / discovery / agent detection.
    pub(crate) fn test_app(workspaces: Vec<Workspace>, sessions: Vec<Session>) -> App {
        let mut app = App {
            sessions: SessionStore {
                workspaces,
                sessions,
                ..Default::default()
            },
            ..Default::default()
        };
        app.rebuild_tree();
        if !app.sessions.tree.is_empty() {
            app.sessions.tree_state.select(Some(0));
        }
        app
    }

    fn ws(id: &str, name: &str, path: &str) -> Workspace {
        Workspace {
            id: id.into(),
            name: name.into(),
            path: Some(PathBuf::from(path)),
            created_at: 1000,
            expanded: true,
        }
    }

    fn sess(id: &str, title: &str, ws_path: &str) -> Session {
        Session { id: id.into(), workspace_path: PathBuf::from(ws_path), title: title.into(), last_active: 1000, agent: Agent::Claude, tags: Vec::new(), pinned: false, last_message: None }
    }

    fn sess_with_agent(id: &str, title: &str, ws_path: &str, agent: Agent) -> Session {
        Session {
            id: id.into(),
            workspace_path: PathBuf::from(ws_path),
            title: title.into(),
            last_active: 1000,
            agent,
            tags: Vec::new(),
            pinned: false,
            last_message: None,
        }
    }

    fn sess_with_time(id: &str, title: &str, ws_path: &str, last_active: u64) -> Session {
        Session {
            id: id.into(),
            workspace_path: PathBuf::from(ws_path),
            title: title.into(),
            last_active,
            agent: Agent::Claude,
            tags: Vec::new(),
            pinned: false,
            last_message: None,
        }
    }

    // ─── session_fuzzy_score tests ───

    #[test]
    fn fuzzy_score_exact_match() {
        assert!(session_fuzzy_score("fix bug", "", "fix bug"));
    }

    #[test]
    fn fuzzy_score_substring_match() {
        assert!(session_fuzzy_score("fix login bug", "", "fix"));
    }

    #[test]
    fn fuzzy_score_fuzzy_chars() {
        assert!(session_fuzzy_score("fix login bug", "", "fxlb"));
    }

    #[test]
    fn fuzzy_score_no_match() {
        assert!(!session_fuzzy_score("hello world", "", "zzzzz"));
    }

    #[test]
    fn fuzzy_score_matches_short_id() {
        // Short ID fallback: "abc12345" matches "abc"
        assert!(session_fuzzy_score("unrelated title", "abc12345", "abc"));
    }

    #[test]
    fn fuzzy_score_empty_query() {
        // Empty query should not match (score would be 0)
        assert!(!session_fuzzy_score("some title", "", ""));
    }

    // ─── rebuild_tree filter tests ───

    #[test]
    fn filter_returns_matching_sessions() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess("s1-aaa111", "fix login", "/tmp"),
            sess("s2-bbb222", "add feature", "/tmp"),
            sess("s3-ccc333", "fix logout", "/tmp"),
        ];
        let mut app = test_app(workspaces, sessions);

        // Filter for "fix"
        app.view.search_query = Some("fix".into());
        app.rebuild_tree();

        // Should have: Workspace(w1), Session(w1, s1_idx), Session(w1, s3_idx)
        // s1 is at index 0, s3 is at index 2
        assert_eq!(
            app.sessions.tree.len(),
            3,
            "expected workspace + 2 matching sessions"
        );
        assert!(matches!(app.sessions.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.sessions.tree[1], TreeNode::Session(0, 0))); // s1 (fix login)
        assert!(matches!(app.sessions.tree[2], TreeNode::Session(0, 2))); // s3 (fix logout)
    }

    #[test]
    fn filter_empty_query_shows_all() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![sess("s1", "first", "/tmp"), sess("s2", "second", "/tmp")];
        let app = test_app(workspaces, sessions);

        // No search_query → all sessions visible (workspace expanded)
        assert_eq!(
            app.sessions.tree.len(),
            3,
            "expected workspace + 2 sessions"
        );
    }

    #[test]
    fn filter_no_matches_empty_tree() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![sess("s1", "hello", "/tmp")];
        let mut app = test_app(workspaces, sessions);

        app.view.search_query = Some("zzzzz".into());
        app.rebuild_tree();

        assert!(
            app.sessions.tree.is_empty(),
            "no matches should yield empty tree"
        );
    }

    #[test]
    fn filter_selection_clamped() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess("s1", "alpha", "/tmp"),
            sess("s2", "beta", "/tmp"),
            sess("s3", "gamma", "/tmp"),
        ];
        let mut app = test_app(workspaces, sessions);

        // Select last item (index 3 = session s3)
        app.sessions.tree_state.select(Some(3));

        // Filter to just one match ("beta")
        app.view.search_query = Some("beta".into());
        app.rebuild_tree();

        // Tree now has only 2 items: Workspace + Session(s2)
        // Selection should be clamped to valid range
        let sel = app.sessions.tree_state.selected();
        assert!(sel.is_some(), "selection must exist when tree is non-empty");
        let idx = sel.unwrap();
        assert!(
            idx < app.sessions.tree.len(),
            "selection ({}) must be < tree len ({})",
            idx,
            app.sessions.tree.len()
        );
    }

    #[test]
    fn filter_restores_all_on_clear() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess("s1", "fix bug", "/tmp"),
            sess("s2", "add feature", "/tmp"),
        ];
        let mut app = test_app(workspaces, sessions);

        // Filter
        app.view.search_query = Some("fix".into());
        app.rebuild_tree();
        assert_eq!(app.sessions.tree.len(), 2, "workspace + 1 matching session");

        // Clear filter
        app.view.search_query = None;
        app.rebuild_tree();
        assert_eq!(
            app.sessions.tree.len(),
            3,
            "workspace + all sessions restored"
        );
    }

    #[test]
    fn filter_workspaces_independently() {
        let tmp = std::env::temp_dir();
        let path_a = tmp.join("amux_test_alpha");
        let path_b = tmp.join("amux_test_beta");
        let _ = std::fs::create_dir_all(&path_a);
        let _ = std::fs::create_dir_all(&path_b);
        let pa = path_a.to_str().unwrap();
        let pb = path_b.to_str().unwrap();
        let workspaces = vec![ws("w1", "Alpha", pa), ws("w2", "Beta", pb)];
        let sessions = vec![
            sess("s1", "fix alpha bug", pa),
            sess("s2", "fix beta bug", pb),
        ];
        let mut app = test_app(workspaces, sessions);

        // Both workspaces should be present unfiltered
        assert_eq!(app.sessions.tree.len(), 4, "2 workspaces + 2 sessions");

        // Filter for "beta"
        app.view.search_query = Some("beta".into());
        app.rebuild_tree();

        // Should have: w1 matching (Beta in name? no) — actually w1 has name "Alpha"
        // s1 title is "fix alpha bug" — fuzzy match "beta"? No.
        // w2 name is "Beta" — matches "beta"
        // s2 title is "fix beta bug" — matches "beta"
        // So: TreeNode::Workspace(1), TreeNode::Session(1, 1)
        assert_eq!(
            app.sessions.tree.len(),
            2,
            "workspace Beta + 1 matching session"
        );
        assert!(matches!(app.sessions.tree[0], TreeNode::Workspace(1)));
        assert!(matches!(app.sessions.tree[1], TreeNode::Session(1, 1)));
    }

    // ─── agent filter tests ───

    #[test]
    fn agent_filter_shows_only_matching_sessions() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess_with_agent("s1", "claude task", "/tmp", Agent::Claude),
            sess_with_agent("s2", "codex task", "/tmp", Agent::Codex),
            sess_with_agent("s3", "omp task", "/tmp", Agent::Omp),
        ];
        let mut app = test_app(workspaces, sessions);

        // Filter to Claude only
        app.view.agent_filter = Some(Agent::Claude);
        app.rebuild_tree();

        // Should have: Workspace(w1), Session(w1, 0) — only Claude session
        assert_eq!(app.sessions.tree.len(), 2, "workspace + 1 Claude session");
        assert!(matches!(app.sessions.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.sessions.tree[1], TreeNode::Session(0, 0))); // s1 (Claude)
    }

    #[test]
    fn agent_filter_hides_non_matching_sessions() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess_with_agent("s1", "claude task", "/tmp", Agent::Claude),
            sess_with_agent("s2", "codex task", "/tmp", Agent::Codex),
        ];
        let mut app = test_app(workspaces, sessions);

        // Unfiltered: workspace + 2 sessions
        assert_eq!(
            app.sessions.tree.len(),
            3,
            "workspace + 2 sessions unfiltered"
        );

        // Filter to OMP (none exist)
        app.view.agent_filter = Some(Agent::Omp);
        app.rebuild_tree();

        // Without search query, workspace is still shown but has no sessions under it
        assert_eq!(
            app.sessions.tree.len(),
            1,
            "workspace header present, no matching sessions listed"
        );
        assert!(matches!(app.sessions.tree[0], TreeNode::Workspace(0)));
    }

    #[test]
    fn agent_filter_combined_with_text_search() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess_with_agent("s1", "fix bug", "/tmp", Agent::Claude),
            sess_with_agent("s2", "fix bug", "/tmp", Agent::Codex),
            sess_with_agent("s3", "add feature", "/tmp", Agent::Claude),
            sess_with_agent("s4", "fix bug", "/tmp", Agent::Omp),
        ];
        let mut app = test_app(workspaces, sessions);

        // Filter to Claude + search "fix"
        app.view.agent_filter = Some(Agent::Claude);
        app.view.search_query = Some("fix".into());
        app.rebuild_tree();

        // Should have: Workspace(w1), Session(w1, 0) — only Claude session matching "fix"
        // s1 (Claude, "fix bug") ✓  s3 (Claude, "add feature") ✗
        assert_eq!(
            app.sessions.tree.len(),
            2,
            "workspace + 1 Claude session matching 'fix'"
        );
        assert!(matches!(app.sessions.tree[1], TreeNode::Session(0, 0))); // s1
    }

    #[test]
    fn toggle_same_agent_key_clears_filter() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess_with_agent("s1", "task a", "/tmp", Agent::Claude),
            sess_with_agent("s2", "task b", "/tmp", Agent::Omp),
        ];
        let mut app = test_app(workspaces, sessions);

        // Set filter to Claude
        app.toggle_agent_filter(Agent::Claude);
        assert_eq!(app.view.agent_filter, Some(Agent::Claude));
        assert_eq!(app.sessions.tree.len(), 2, "workspace + 1 Claude session");

        // Toggle same agent again should clear
        app.toggle_agent_filter(Agent::Claude);
        assert_eq!(app.view.agent_filter, None);
        assert_eq!(
            app.sessions.tree.len(),
            3,
            "workspace + all 2 sessions restored"
        );
    }

    // ─── sort mode tests ───

    #[test]
    fn sort_mode_cycles_through_all_variants() {
        let start = SortMode::TimeDesc;
        let mut mode = start;
        let mut visited = vec![mode];
        for _ in 0..4 {
            mode = mode.next();
            visited.push(mode);
        }
        // After 5 steps we should be back at the start
        let wrapped = mode.next();
        assert_eq!(wrapped, start, "sort mode should wrap to TimeDesc");

        // All 5 variants visited
        assert_eq!(visited.len(), 5);
        assert!(visited.contains(&SortMode::TimeDesc));
        assert!(visited.contains(&SortMode::TimeAsc));
        assert!(visited.contains(&SortMode::NameAsc));
        assert!(visited.contains(&SortMode::NameDesc));
        assert!(visited.contains(&SortMode::AgentGroup));
    }

    #[test]
    fn sort_mode_default_is_time_desc() {
        assert_eq!(SortMode::default(), SortMode::TimeDesc);
    }

    #[test]
    fn sort_time_desc_newest_first() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess_with_time("s1", "old", "/tmp", 100),
            sess_with_time("s2", "mid", "/tmp", 500),
            sess_with_time("s3", "new", "/tmp", 900),
        ];
        let mut app = test_app(workspaces, sessions);
        app.view.sort_mode = SortMode::TimeDesc;
        app.rebuild_tree();

        // Tree: [Workspace(0), Session(0,2), Session(0,1), Session(0,0)]
        // (newest first: s3=900, s2=500, s1=100)
        assert!(matches!(app.sessions.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.sessions.tree[1], TreeNode::Session(0, 2))); // newest
        assert!(matches!(app.sessions.tree[2], TreeNode::Session(0, 1)));
        assert!(matches!(app.sessions.tree[3], TreeNode::Session(0, 0))); // oldest
    }

    #[test]
    fn sort_time_asc_oldest_first() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess_with_time("s1", "old", "/tmp", 100),
            sess_with_time("s2", "mid", "/tmp", 500),
            sess_with_time("s3", "new", "/tmp", 900),
        ];
        let mut app = test_app(workspaces, sessions);
        app.view.sort_mode = SortMode::TimeAsc;
        app.rebuild_tree();

        // Tree: [Workspace(0), Session(0,0), Session(0,1), Session(0,2)]
        // (oldest first: s1=100, s2=500, s3=900)
        assert!(matches!(app.sessions.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.sessions.tree[1], TreeNode::Session(0, 0))); // oldest
        assert!(matches!(app.sessions.tree[2], TreeNode::Session(0, 1)));
        assert!(matches!(app.sessions.tree[3], TreeNode::Session(0, 2))); // newest
    }

    #[test]
    fn sort_name_asc_alphabetical() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess("s1", "Charlie", "/tmp"),
            sess("s2", "alpha", "/tmp"),
            sess("s3", "Bravo", "/tmp"),
        ];
        let mut app = test_app(workspaces, sessions);
        app.view.sort_mode = SortMode::NameAsc;
        app.rebuild_tree();

        // Case-insensitive: alpha < Bravo < Charlie
        // s2=alpha(idx1), s3=Bravo(idx2), s1=Charlie(idx0)
        assert!(matches!(app.sessions.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.sessions.tree[1], TreeNode::Session(0, 1))); // alpha
        assert!(matches!(app.sessions.tree[2], TreeNode::Session(0, 2))); // Bravo
        assert!(matches!(app.sessions.tree[3], TreeNode::Session(0, 0))); // Charlie
    }

    #[test]
    fn sort_name_desc_reverse_alphabetical() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess("s1", "Charlie", "/tmp"),
            sess("s2", "alpha", "/tmp"),
            sess("s3", "Bravo", "/tmp"),
        ];
        let mut app = test_app(workspaces, sessions);
        app.view.sort_mode = SortMode::NameDesc;
        app.rebuild_tree();

        // Case-insensitive reverse: Charlie > Bravo > alpha
        // s1=Charlie(idx0), s3=Bravo(idx2), s2=alpha(idx1)
        assert!(matches!(app.sessions.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.sessions.tree[1], TreeNode::Session(0, 0))); // Charlie
        assert!(matches!(app.sessions.tree[2], TreeNode::Session(0, 2))); // Bravo
        assert!(matches!(app.sessions.tree[3], TreeNode::Session(0, 1))); // alpha
    }

    #[test]
    fn sort_agent_group_groups_by_agent() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess_with_agent("s1", "task 1", "/tmp", Agent::Omp),
            sess_with_agent("s2", "task 2", "/tmp", Agent::Claude),
            sess_with_agent("s3", "task 3", "/tmp", Agent::Codex),
        ];
        let mut app = test_app(workspaces, sessions);
        app.view.sort_mode = SortMode::AgentGroup;
        app.rebuild_tree();

        // Expected tree: Workspace(0), AgentHeader(Claude), Session(Claude),
        //                AgentHeader(Codex), Session(Codex), AgentHeader(Omp), Session(Omp)
        assert!(matches!(app.sessions.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(
            app.sessions.tree[1],
            TreeNode::AgentHeader(Agent::Claude)
        ));
        assert!(matches!(app.sessions.tree[2], TreeNode::Session(0, 1))); // s2 = Claude
        assert!(matches!(
            app.sessions.tree[3],
            TreeNode::AgentHeader(Agent::Codex)
        ));
        assert!(matches!(app.sessions.tree[4], TreeNode::Session(0, 2))); // s3 = Codex
        assert!(matches!(
            app.sessions.tree[5],
            TreeNode::AgentHeader(Agent::Omp)
        ));
        assert!(matches!(app.sessions.tree[6], TreeNode::Session(0, 0))); // s1 = Omp
    }

    #[test]
    fn sort_agent_group_omits_empty_groups() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess_with_agent("s1", "task 1", "/tmp", Agent::Claude),
            sess_with_agent("s2", "task 2", "/tmp", Agent::Claude),
        ];
        let mut app = test_app(workspaces, sessions);
        app.view.sort_mode = SortMode::AgentGroup;
        app.rebuild_tree();

        // Only Claude sessions — no Codex or Omp headers
        assert_eq!(
            app.sessions.tree.len(),
            4,
            "workspace + header + 2 sessions"
        );
        assert!(matches!(app.sessions.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(
            app.sessions.tree[1],
            TreeNode::AgentHeader(Agent::Claude)
        ));
        assert!(matches!(app.sessions.tree[2], TreeNode::Session(0, 0)));
        assert!(matches!(app.sessions.tree[3], TreeNode::Session(0, 1)));

        // No other agent headers
        for node in &app.sessions.tree {
            if let TreeNode::AgentHeader(a) = node {
                assert_eq!(*a, Agent::Claude, "only Claude header should appear");
            }
        }
    }

    #[test]
    fn sort_with_active_filter() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess_with_time("s1", "fix alpha", "/tmp", 100),
            sess_with_time("s2", "fix beta", "/tmp", 500),
            sess_with_time("s3", "add feature", "/tmp", 900),
        ];
        let mut app = test_app(workspaces, sessions);

        // Apply filter + sort
        app.view.search_query = Some("fix".into());
        app.view.sort_mode = SortMode::TimeAsc;
        app.rebuild_tree();

        // Filter matches s1 and s2; TimeAsc puts oldest first
        // Tree: [Workspace(0), Session(0,0) s1=100, Session(0,1) s2=500]
        assert!(matches!(app.sessions.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.sessions.tree[1], TreeNode::Session(0, 0))); // s1 oldest
        assert!(matches!(app.sessions.tree[2], TreeNode::Session(0, 1))); // s2 newer
    }

    #[test]
    fn agent_header_is_inert_for_activate() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess_with_agent("s1", "task", "/tmp", Agent::Claude),
            sess_with_agent("s2", "task", "/tmp", Agent::Codex),
        ];
        let mut app = test_app(workspaces, sessions);
        app.view.sort_mode = SortMode::AgentGroup;
        app.rebuild_tree();

        // Find an AgentHeader node index
        let header_idx = app
            .sessions
            .tree
            .iter()
            .position(|n| matches!(n, TreeNode::AgentHeader(_)))
            .expect("should have an AgentHeader");
        app.sessions.tree_state.select(Some(header_idx));

        let focus_before = app.view.focus;
        let mode_before = app.view.input_mode;
        let result = app.activate_selection();
        assert!(
            result.is_ok(),
            "activate_selection on AgentHeader should succeed"
        );
        assert_eq!(app.view.focus, focus_before, "focus should not change");
        assert_eq!(
            app.view.input_mode, mode_before,
            "input_mode should not change"
        );
    }

    #[test]
    fn agent_header_is_inert_for_delete() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess_with_agent("s1", "task", "/tmp", Agent::Claude),
            sess_with_agent("s2", "task", "/tmp", Agent::Codex),
        ];
        let mut app = test_app(workspaces, sessions);
        app.view.sort_mode = SortMode::AgentGroup;
        app.rebuild_tree();

        let session_count_before = app.sessions.sessions.len();
        let tree_len_before = app.sessions.tree.len();

        // Select an AgentHeader
        let header_idx = app
            .sessions
            .tree
            .iter()
            .position(|n| matches!(n, TreeNode::AgentHeader(_)))
            .expect("should have an AgentHeader");
        app.sessions.tree_state.select(Some(header_idx));
        app.request_delete();

        // Nothing should be deleted
        assert_eq!(
            app.sessions.sessions.len(),
            session_count_before,
            "no session should be deleted"
        );
        assert_eq!(
            app.sessions.tree.len(),
            tree_len_before,
            "tree should not change after deleting AgentHeader"
        );
    }

    #[test]
    fn sort_preserves_selection_clamping() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![
            sess_with_time("s1", "old", "/tmp", 100),
            sess_with_time("s2", "mid", "/tmp", 500),
            sess_with_time("s3", "new", "/tmp", 900),
        ];
        let mut app = test_app(workspaces, sessions);

        // Select last session (index 3 = session s3)
        app.sessions.tree_state.select(Some(3));
        assert_eq!(app.sessions.tree_state.selected(), Some(3));

        // Switch to AgentGroup mode which adds AgentHeader nodes (tree grows)
        app.view.sort_mode = SortMode::AgentGroup;
        app.rebuild_tree();

        // Selection should be clamped to valid range
        let sel = app.sessions.tree_state.selected();
        assert!(sel.is_some(), "selection must exist when tree non-empty");
        let idx = sel.unwrap();
        assert!(
            idx < app.sessions.tree.len(),
            "selection ({}) must be < tree len ({})",
            idx,
            app.sessions.tree.len()
        );
    }

    // ─── handle_mouse_click tests ───

    #[test]
    fn mouse_click_ignored_when_no_ptys() {
        let mut app = test_app(vec![], vec![]);
        app.view.tab_bar_rect = Rect::new(0, 0, 80, 1);
        // Should not panic or change state
        app.handle_mouse_click(40, 0);
        assert_eq!(app.ptys.active_pty, None);
    }

    #[test]
    fn mouse_click_ignored_when_rect_is_zero() {
        let mut app = test_app(vec![], vec![]);
        app.view.tab_bar_rect = Rect::default();
        app.handle_mouse_click(10, 10);
        assert_eq!(app.ptys.active_pty, None);
    }

    #[test]
    fn mouse_click_outside_tab_bar_ignored() {
        let mut app = test_app(vec![], vec![]);
        app.view.tab_bar_rect = Rect::new(0, 0, 80, 1);

        // Click below the tab bar (y=1 is outside a rect starting at y=0 with height=1)
        app.handle_mouse_click(40, 5);
        assert_eq!(app.ptys.active_pty, None);

        // Click above the tab bar
        app.handle_mouse_click(40, 10);
        assert_eq!(app.ptys.active_pty, None);
    }

    #[test]
    #[allow(clippy::erasing_op, clippy::identity_op)]
    fn tab_index_calculation_single_tab() {
        // With 1 tab spanning width=80, tab_width=80, any click maps to index 0
        let rect = Rect::new(0, 0, 80, 1);
        let tab_width = rect.width / 1u16; // 80
        assert_eq!(tab_width, 80);

        // Click at x=0 → local_x=0 → index 0
        let idx = (0u16 / tab_width) as usize;
        assert_eq!(idx, 0);

        // Click at x=79 → local_x=79 → index 0
        let idx = (79u16 / tab_width) as usize;
        assert_eq!(idx, 0);
    }

    #[test]
    #[allow(clippy::erasing_op)]
    fn tab_index_calculation_multiple_tabs() {
        // With 4 tabs spanning width=80, tab_width=20
        let rect = Rect::new(0, 0, 80, 1);
        let tab_width = rect.width / 4u16;
        assert_eq!(tab_width, 20);

        // Click at x=0 → index 0
        assert_eq!((0u16 / tab_width) as usize, 0);
        // Click at x=19 → index 0
        assert_eq!((19u16 / tab_width) as usize, 0);
        // Click at x=20 → index 1
        assert_eq!((20u16 / tab_width) as usize, 1);
        // Click at x=39 → index 1
        assert_eq!((39u16 / tab_width) as usize, 1);
        // Click at x=40 → index 2
        assert_eq!((40u16 / tab_width) as usize, 2);
        // Click at x=60 → index 3
        assert_eq!((60u16 / tab_width) as usize, 3);
        // Click at x=79 → index 3
        assert_eq!((79u16 / tab_width) as usize, 3);
    }

    #[test]
    fn tab_index_with_offset_rect() {
        // Tab bar at x=20, width=60, 3 tabs → tab_width=20
        let rect = Rect::new(20, 5, 60, 1);
        let tab_width = rect.width / 3u16;
        assert_eq!(tab_width, 20);

        // Click at x=20 → local_x=0 → index 0
        let local_x = 20u16 - rect.x;
        assert_eq!((local_x / tab_width) as usize, 0);

        // Click at x=39 → local_x=19 → index 0
        let local_x = 39u16 - rect.x;
        assert_eq!((local_x / tab_width) as usize, 0);

        // Click at x=40 → local_x=20 → index 1
        let local_x = 40u16 - rect.x;
        assert_eq!((local_x / tab_width) as usize, 1);

        // Click at x=59 → local_x=39 → index 1
        let local_x = 59u16 - rect.x;
        assert_eq!((local_x / tab_width) as usize, 1);

        // Click at x=60 → local_x=40 → index 2
        let local_x = 60u16 - rect.x;
        assert_eq!((local_x / tab_width) as usize, 2);
    }

    #[test]
    fn archive_auto_filters_old_sessions() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let now = now_secs();
        let old_time = now - 8 * 86400; // 8 days ago
        let recent_time = now - 3600; // 1 hour ago
        let sessions = vec![
            Session {
                id: "old1".into(),
                workspace_path: PathBuf::from("/tmp"),
                title: "Old session".into(),
                last_active: old_time,
                agent: Agent::Claude,
                tags: Vec::new(),
                pinned: false,
                last_message: None,
            },
            Session {
                id: "new1".into(),
                workspace_path: PathBuf::from("/tmp"),
                title: "Recent session".into(),
                last_active: recent_time,
                agent: Agent::Claude,
                tags: Vec::new(),
                pinned: false,
                last_message: None,
            },
        ];
        let mut app = test_app(workspaces, sessions);
        app.sessions.archive_days = Some(7);

        // Archive runs automatically in refresh, but we call it directly to test
        app.archive_old_sessions();

        // Old session should be filtered from active sessions
        assert_eq!(app.sessions.sessions.len(), 1);
        assert_eq!(app.sessions.sessions[0].id, "new1");

        // Old session should be in archived
        assert_eq!(app.sessions.archived_sessions.len(), 1);
        assert_eq!(app.sessions.archived_sessions[0].id, "old1");

        // Tree should not contain archived by default
        assert!(app.sessions.tree.iter().all(|n| !matches!(
            n,
            TreeNode::ArchivedHeader | TreeNode::ArchivedSession(_, _)
        )));
    }

    #[test]
    fn archive_toggle_shows_archived_in_tree() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let now = now_secs();
        let old_time = now - 8 * 86400;
        let sessions = vec![Session {
            id: "old1".into(),
            workspace_path: PathBuf::from("/tmp"),
            title: "Old".into(),
            last_active: old_time,
            agent: Agent::Claude,
            tags: Vec::new(),
            pinned: false,
            last_message: None,
        }];
        let mut app = test_app(workspaces, sessions);
        app.sessions.archive_days = Some(7);
        app.archive_old_sessions();
        assert_eq!(app.sessions.archived_sessions.len(), 1);

        // Toggle on
        app.sessions.show_archived = true;
        app.rebuild_tree();
        assert!(
            app.sessions
                .tree
                .iter()
                .any(|n| matches!(n, TreeNode::ArchivedHeader))
        );
        assert!(
            app.sessions
                .tree
                .iter()
                .any(|n| matches!(n, TreeNode::ArchivedSession(_, _)))
        );

        // Toggle off
        app.sessions.show_archived = false;
        app.rebuild_tree();
        assert!(
            !app.sessions
                .tree
                .iter()
                .any(|n| matches!(n, TreeNode::ArchivedHeader))
        );
    }

    #[test]
    fn archive_restores_sessions_when_threshold_changes() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let now = now_secs();
        let old_time = now - 5 * 86400; // 5 days ago
        let sessions = vec![Session {
            id: "s1".into(),
            workspace_path: PathBuf::from("/tmp"),
            title: "Five days".into(),
            last_active: old_time,
            agent: Agent::Claude,
            tags: Vec::new(),
            pinned: false,
            last_message: None,
        }];
        let mut app = test_app(workspaces, sessions);
        app.sessions.archive_days = Some(3);
        app.archive_old_sessions();
        assert_eq!(app.sessions.archived_sessions.len(), 1);
        assert_eq!(app.sessions.sessions.len(), 0);

        // Increase threshold to 7 days — session should be restored
        app.sessions.archive_days = Some(7);
        app.archive_old_sessions();
        assert_eq!(app.sessions.archived_sessions.len(), 0);
        assert_eq!(app.sessions.sessions.len(), 1);
        assert_eq!(app.sessions.sessions[0].id, "s1");
    }

    #[test]
    fn archive_no_files_deleted() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let now = now_secs();
        let old_time = now - 10 * 86400;
        let sessions = vec![Session {
            id: "old1".into(),
            workspace_path: PathBuf::from("/tmp"),
            title: "Old".into(),
            last_active: old_time,
            agent: Agent::Claude,
            tags: Vec::new(),
            pinned: false,
            last_message: None,
        }];
        let mut app = test_app(workspaces, sessions);
        app.sessions.archive_days = Some(7);

        // Before archive
        assert_eq!(app.sessions.sessions.len(), 1);

        app.archive_old_sessions();

        // Session moved to archived, not removed from memory
        assert_eq!(app.sessions.archived_sessions.len(), 1);
        // The archived session data is preserved intact
        assert_eq!(app.sessions.archived_sessions[0].id, "old1");
        assert_eq!(app.sessions.archived_sessions[0].title, "Old");
    }
}
