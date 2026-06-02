use std::{
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
};

use crate::config::{data_dir, save_config_file, title_override_path};
use crate::discovery::{discover_sessions, find_session_jsonl};
use crate::pty::PtyState;
use crate::types::*;
use crate::util::*;
use anyhow::{Context, Result};
use crossterm::event::{Event, KeyEventKind};
use ratatui::{layout::Rect, widgets::ListState};

struct App {
    workspaces: Vec<Workspace>,
    sessions: Vec<Session>,
    tree: Vec<TreeNode>,
    ws_session_map: Vec<Vec<usize>>,
    tree_state: ListState,
    focus: Focus,
    input_mode: InputMode,
    input_buffer: String,
    search_query: Option<String>,
    rename_target: Option<usize>,
    rename_workspace_target: Option<usize>,
    new_workspace_name: Option<String>,
    pending_session_name: Option<String>,
    available_agents: Vec<Agent>,
    agent_state: ListState,
    browse_dir: PathBuf,
    browse_entries: Vec<DirEntry>,
    browse_state: ListState,
    ptys: Vec<PtySlot>,
    active_pty: Option<usize>,
    status: String,
    last_chat_area: Rect,
    tab_bar_rect: Rect,
    last_refresh: std::time::Instant,
    prev_states: Vec<PtyState>,
    agent_filter: Option<Agent>,
    sort_mode: SortMode,
}

mod browse;
mod handler;
mod session;
mod ui;

impl App {
    fn new() -> Self {
        let mut config = crate::config::load_config().unwrap_or_else(|_| Config {
            workspaces: Vec::new(),
        });

        if config.workspaces.is_empty() {
            config.workspaces = crate::discovery::discover_workspaces_from_fs();
            let _ = save_config_file(&config);
        }

        for ws in &mut config.workspaces {
            ws.expanded = true;
        }

        let sessions = discover_sessions(&config.workspaces);
        let mut app = Self {
            workspaces: config.workspaces,
            sessions,
            tree: Vec::new(),
            ws_session_map: Vec::new(),
            tree_state: ListState::default(),
            focus: Focus::Sidebar,
            input_mode: InputMode::None,
            input_buffer: String::new(),
            search_query: None,
            rename_target: None,
            rename_workspace_target: None,
            new_workspace_name: None,
            pending_session_name: None,
            available_agents: detect_agents(),
            agent_state: ListState::default(),
            browse_dir: PathBuf::new(),
            browse_entries: Vec::new(),
            browse_state: ListState::default(),
            ptys: Vec::new(),
            active_pty: None,
            status: "Enter:new/resume e:expand r:refresh R:rename N:new-ws D:del-ws q:quit".into(),
            last_chat_area: Rect::default(),
            tab_bar_rect: Rect::default(),
            last_refresh: std::time::Instant::now(),
            prev_states: Vec::new(),
            agent_filter: None,
            sort_mode: SortMode::default(),
        };
        app.rebuild_tree();
        if !app.tree.is_empty() {
            app.tree_state.select(Some(0));
        }
        app
    }

    fn cycle_sort_mode(&mut self) {
        self.sort_mode = self.sort_mode.next();
        self.rebuild_tree();
        self.status = format!("Sort: {}", self.sort_mode.label());
    }

    fn sort_session_indices(&self, indices: &mut [usize]) {
        match self.sort_mode {
            SortMode::TimeDesc => indices.sort_by(|&a, &b| {
                self.sessions[b]
                    .last_active
                    .cmp(&self.sessions[a].last_active)
            }),
            SortMode::TimeAsc => indices.sort_by(|&a, &b| {
                self.sessions[a]
                    .last_active
                    .cmp(&self.sessions[b].last_active)
            }),
            SortMode::NameAsc => indices.sort_by(|&a, &b| {
                self.sessions[a]
                    .title
                    .to_lowercase()
                    .cmp(&self.sessions[b].title.to_lowercase())
            }),
            SortMode::NameDesc => indices.sort_by(|&a, &b| {
                self.sessions[b]
                    .title
                    .to_lowercase()
                    .cmp(&self.sessions[a].title.to_lowercase())
            }),
            SortMode::AgentGroup => indices.sort_by(|&a, &b| {
                self.sessions[a]
                    .agent
                    .cmp(&self.sessions[b].agent)
                    .then_with(|| {
                        self.sessions[b]
                            .last_active
                            .cmp(&self.sessions[a].last_active)
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
        let agent_order = [Agent::Claude, Agent::Codex, Agent::Gsd];
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
        for slot in &mut self.ptys {
            let state = slot.handle.state();
            if state == PtyState::Running {
                slot.info.completed = false;
            } else if !slot.info.completed && state == PtyState::Completed {
                slot.info.completed = true;
            }
        }

        let before = self.ptys.len();
        self.ptys.retain(|slot| {
            if slot.info.agent == Agent::Codex && !slot.handle.is_alive() {
                return false;
            }
            true
        });
        if self.ptys.len() != before {
            if let Some(cur) = self.active_pty
                && cur >= self.ptys.len()
            {
                self.active_pty = if self.ptys.is_empty() {
                    None
                } else {
                    Some(self.ptys.len() - 1)
                };
            }
            self.rebuild_tree();
        }

        self.prev_states = self.ptys.iter().map(|s| s.handle.state()).collect();
    }

    fn pty_display_state(&self, pi: usize) -> PtyState {
        if let Some(slot) = self.ptys.get(pi) {
            slot.handle.state()
        } else {
            PtyState::Running
        }
    }

    fn toggle_agent_filter(&mut self, agent: Agent) {
        if self.agent_filter == Some(agent) {
            self.agent_filter = None;
            self.status = "Filter: all agents".to_string();
        } else {
            self.agent_filter = Some(agent);
            self.status = format!("Filter: {}", agent.label());
        }
        self.rebuild_tree();
    }

    fn refresh_sessions(&mut self) {
        self.sessions = discover_sessions(&self.workspaces);

        for slot in &mut self.ptys {
            if slot.info.session_id.is_none()
                && let Some(found) = self.sessions.iter().find(|s| {
                    s.workspace_path == slot.info.workspace_path
                        && s.last_active >= slot.info.started_at
                })
            {
                slot.info.session_id = Some(found.id.clone());
            }
        }

        self.rebuild_tree();
    }

    fn rebuild_tree(&mut self) {
        let mut tree = Vec::new();
        let mut ws_map = Vec::new();
        let query = self
            .search_query
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty());

        for (wi, _ws) in self.workspaces.iter().enumerate() {
            let sess_idxs: Vec<usize> = self
                .sessions
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    self.ws_matches_path(wi, &s.workspace_path)
                        && self.agent_filter.is_none_or(|agent| s.agent == agent)
                })
                .map(|(i, _)| i)
                .collect();

            if let Some(q) = query {
                // Fuzzy-filter sessions for this workspace
                let mut matching_sessions: Vec<usize> = sess_idxs
                    .into_iter()
                    .filter(|&si| {
                        let session = &self.sessions[si];
                        let short_id = &session.id[..session.id.len().min(8)];
                        session_fuzzy_score(session.title.as_str(), short_id, q)
                            || session_fuzzy_score(&self.workspaces[wi].name, short_id, q)
                    })
                    .collect();
                self.sort_session_indices(&mut matching_sessions);

                // Fuzzy-filter active PTYs for this workspace
                let matching_ptys: Vec<usize> = self
                    .ptys
                    .iter()
                    .enumerate()
                    .filter(|(_pi, slot)| {
                        self.ws_matches_path(wi, &slot.info.workspace_path)
                            && slot.info.session_id.is_none()
                            && self.agent_filter.is_none_or(|a| slot.info.agent == a)
                            && session_fuzzy_score(&slot.info.title, &slot.info.title, q)
                    })
                    .map(|(pi, _)| pi)
                    .collect();

                // Include workspace only if it matches itself or has matching children
                let ws_matches = session_fuzzy_score(&self.workspaces[wi].name, "", q);
                if ws_matches || !matching_sessions.is_empty() || !matching_ptys.is_empty() {
                    tree.push(TreeNode::Workspace(wi));
                    for &pi in &matching_ptys {
                        tree.push(TreeNode::ActiveTab(pi));
                    }
                    if self.sort_mode == SortMode::AgentGroup {
                        Self::append_agent_grouped(
                            &self.sessions,
                            &matching_sessions,
                            wi,
                            &mut tree,
                        );
                    } else {
                        for &si in &matching_sessions {
                            tree.push(TreeNode::Session(wi, si));
                        }
                    }
                }
                ws_map.push(matching_sessions);
            } else {
                let mut sorted_idxs = sess_idxs.clone();
                self.sort_session_indices(&mut sorted_idxs);
                tree.push(TreeNode::Workspace(wi));
                if self.workspaces[wi].expanded {
                    for (pi, slot) in self.ptys.iter().enumerate() {
                        if self.ws_matches_path(wi, &slot.info.workspace_path)
                            && slot.info.session_id.is_none()
                            && self.agent_filter.is_none_or(|a| slot.info.agent == a)
                        {
                            tree.push(TreeNode::ActiveTab(pi));
                        }
                    }
                    if self.sort_mode == SortMode::AgentGroup {
                        Self::append_agent_grouped(&self.sessions, &sorted_idxs, wi, &mut tree);
                    } else {
                        for &si in &sorted_idxs {
                            tree.push(TreeNode::Session(wi, si));
                        }
                    }
                }
                ws_map.push(sess_idxs);
            }
        }

        self.tree = tree;
        self.ws_session_map = ws_map;

        // Clamp selection to valid range
        if !self.tree.is_empty() {
            self.move_sel(0);
        }
    }

    fn pty_index_for_session(&self, session_id: &str) -> Option<usize> {
        self.ptys
            .iter()
            .position(|s| s.info.session_id.as_deref() == Some(session_id))
    }

    fn selected_node(&self) -> Option<&TreeNode> {
        self.tree_state.selected().and_then(|i| self.tree.get(i))
    }

    fn move_sel(&mut self, delta: isize) {
        let len = self.tree.len();
        if len == 0 {
            return;
        }
        let cur = self.tree_state.selected().unwrap_or(0).min(len - 1) as isize;
        self.tree_state
            .select(Some(((cur + delta).rem_euclid(len as isize)) as usize));
    }

    fn toggle_expand(&mut self) {
        if let Some(TreeNode::Workspace(wi)) = self.selected_node() {
            let wi = *wi;
            self.workspaces[wi].expanded = !self.workspaces[wi].expanded;
            self.rebuild_tree();
        }
    }

    fn delete_selected(&mut self) {
        match self.selected_node().cloned() {
            Some(TreeNode::Workspace(wi)) => {
                let name = self.workspaces[wi].name.clone();
                self.workspaces.remove(wi);
                self.save_config();
                self.refresh_sessions();
                self.status = format!("Deleted workspace: {}", name);
            }
            Some(TreeNode::Session(_wi, si)) => {
                if si >= self.sessions.len() {
                    return;
                }
                let session = self.sessions[si].clone();
                if let Some(pi) = self.pty_index_for_session(&session.id) {
                    self.ptys.remove(pi);
                    if let Some(cur) = self.active_pty
                        && cur >= self.ptys.len()
                    {
                        self.active_pty = if self.ptys.is_empty() {
                            None
                        } else {
                            Some(self.ptys.len() - 1)
                        };
                    }
                }
                let title_path = title_override_path(&session.id);
                let _ = fs::remove_file(&title_path);
                if let Some(jsonl) = find_session_jsonl(&session) {
                    let _ = fs::remove_file(&jsonl);
                }
                let title = session.title.clone();
                self.sessions.remove(si);
                self.rebuild_tree();
                self.status = format!("Deleted session: {}", title);
            }
            Some(TreeNode::AgentHeader(_)) => {}
            Some(TreeNode::ActiveTab(_)) => {}
            None => {}
        }
    }

    fn save_config(&self) {
        let config = Config {
            workspaces: self.workspaces.clone(),
        };
        if let Err(e) = save_config_file(&config) {
            eprintln!("Failed to save config: {}", e);
        }
    }

    fn workspace_cwd(&self, wi: usize) -> PathBuf {
        match &self.workspaces[wi].path {
            Some(p) => p.clone(),
            None => {
                let dir = data_dir().join("workspaces").join(&self.workspaces[wi].id);
                let _ = fs::create_dir_all(&dir);
                dir
            }
        }
    }

    fn ws_matches_path(&self, wi: usize, path: &Path) -> bool {
        match &self.workspaces[wi].path {
            Some(p) => p == path,
            None => path == self.workspace_cwd(wi),
        }
    }

    fn activate_selection(&mut self) -> Result<()> {
        match self.selected_node().cloned() {
            Some(TreeNode::Workspace(_)) => {
                self.input_mode = InputMode::SessionName;
                self.input_buffer.clear();
                self.status = "Enter session name (empty = unnamed, Esc = cancel):".into();
            }
            Some(TreeNode::Session(_wi, si)) => {
                let agent = self.sessions[si].agent;
                self.spawn_with_agent(agent, None)?;
            }
            Some(TreeNode::ActiveTab(pi)) => {
                self.active_pty = Some(pi);
                self.focus = Focus::Chat;
            }
            Some(TreeNode::AgentHeader(_)) => {}
            None => {}
        }
        Ok(())
    }
}

/// Returns true if any of the `haystacks` fuzzy-matches `query` using code_fuzzy_match.
fn session_fuzzy_score(title: &str, short_id: &str, query: &str) -> bool {
    let check = |text: &str| -> bool {
        code_fuzzy_match::fuzzy_match(text, query).is_some_and(|score| score > 0)
    };
    check(title) || check(short_id)
}

// ─── Main ─────────────────────────────────────────────────

pub fn run() -> anyhow::Result<()> {
    let agents = detect_agents();
    if agents.is_empty() {
        anyhow::bail!("No agent CLI found. Install Claude Code, Codex, or GSD.");
    }

    crate::config::ensure_data_dir().context("failed to create data directory")?;

    let mut app = App::new();

    if !std::io::stdout().is_terminal() {
        let sessions = discover_sessions(&app.workspaces);
        for (wi, ws) in app.workspaces.iter().enumerate() {
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

    let result = loop {
        terminal.draw(|frame| app.render(frame))?;

        app.poll_states();

        if !app.ptys.is_empty() && app.last_refresh.elapsed() > std::time::Duration::from_secs(5) {
            app.refresh_sessions();
            app.last_refresh = std::time::Instant::now();
        }

        if crossterm::event::poll(std::time::Duration::from_millis(50))? {
            match crossterm::event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match app.handle_key(key)? {
                    Action::Continue => {}
                    Action::Quit => break Ok(()),
                },
                Event::Paste(text) => {
                    app.handle_paste(&text)?;
                }
                Event::Mouse(mouse) => {
                    if let crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) = mouse.kind {
                        app.handle_mouse_click(mouse.column, mouse.row);
                    }
                }
                _ => {}
            }
        }
    };

    app.ptys.clear();
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
            workspaces,
            sessions,
            tree: Vec::new(),
            ws_session_map: Vec::new(),
            tree_state: ListState::default(),
            focus: Focus::Sidebar,
            input_mode: InputMode::None,
            input_buffer: String::new(),
            search_query: None,
            rename_target: None,
            rename_workspace_target: None,
            new_workspace_name: None,
            pending_session_name: None,
            available_agents: Vec::new(),
            agent_state: ListState::default(),
            browse_dir: PathBuf::new(),
            browse_entries: Vec::new(),
            browse_state: ListState::default(),
            ptys: Vec::new(),
            active_pty: None,
            status: String::new(),
            last_chat_area: Rect::default(),
            tab_bar_rect: Rect::default(),
            last_refresh: std::time::Instant::now(),
            prev_states: Vec::new(),
            agent_filter: None,
            sort_mode: SortMode::default(),
        };
        app.rebuild_tree();
        if !app.tree.is_empty() {
            app.tree_state.select(Some(0));
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
        Session {
            id: id.into(),
            workspace_path: PathBuf::from(ws_path),
            title: title.into(),
            last_active: 1000,
            agent: Agent::Claude,
        }
    }

    fn sess_with_agent(id: &str, title: &str, ws_path: &str, agent: Agent) -> Session {
        Session {
            id: id.into(),
            workspace_path: PathBuf::from(ws_path),
            title: title.into(),
            last_active: 1000,
            agent,
        }
    }

    fn sess_with_time(id: &str, title: &str, ws_path: &str, last_active: u64) -> Session {
        Session {
            id: id.into(),
            workspace_path: PathBuf::from(ws_path),
            title: title.into(),
            last_active,
            agent: Agent::Claude,
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
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess("s1-aaa111", "fix login", "/home/user/proj"),
            sess("s2-bbb222", "add feature", "/home/user/proj"),
            sess("s3-ccc333", "fix logout", "/home/user/proj"),
        ];
        let mut app = test_app(workspaces, sessions);

        // Filter for "fix"
        app.search_query = Some("fix".into());
        app.rebuild_tree();

        // Should have: Workspace(w1), Session(w1, s1_idx), Session(w1, s3_idx)
        // s1 is at index 0, s3 is at index 2
        assert_eq!(
            app.tree.len(),
            3,
            "expected workspace + 2 matching sessions"
        );
        assert!(matches!(app.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.tree[1], TreeNode::Session(0, 0))); // s1 (fix login)
        assert!(matches!(app.tree[2], TreeNode::Session(0, 2))); // s3 (fix logout)
    }

    #[test]
    fn filter_empty_query_shows_all() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess("s1", "first", "/home/user/proj"),
            sess("s2", "second", "/home/user/proj"),
        ];
        let app = test_app(workspaces, sessions);

        // No search_query → all sessions visible (workspace expanded)
        assert_eq!(app.tree.len(), 3, "expected workspace + 2 sessions");
    }

    #[test]
    fn filter_no_matches_empty_tree() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![sess("s1", "hello", "/home/user/proj")];
        let mut app = test_app(workspaces, sessions);

        app.search_query = Some("zzzzz".into());
        app.rebuild_tree();

        assert!(app.tree.is_empty(), "no matches should yield empty tree");
    }

    #[test]
    fn filter_selection_clamped() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess("s1", "alpha", "/home/user/proj"),
            sess("s2", "beta", "/home/user/proj"),
            sess("s3", "gamma", "/home/user/proj"),
        ];
        let mut app = test_app(workspaces, sessions);

        // Select last item (index 3 = session s3)
        app.tree_state.select(Some(3));

        // Filter to just one match ("beta")
        app.search_query = Some("beta".into());
        app.rebuild_tree();

        // Tree now has only 2 items: Workspace + Session(s2)
        // Selection should be clamped to valid range
        let sel = app.tree_state.selected();
        assert!(sel.is_some(), "selection must exist when tree is non-empty");
        let idx = sel.unwrap();
        assert!(
            idx < app.tree.len(),
            "selection ({}) must be < tree len ({})",
            idx,
            app.tree.len()
        );
    }

    #[test]
    fn filter_restores_all_on_clear() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess("s1", "fix bug", "/home/user/proj"),
            sess("s2", "add feature", "/home/user/proj"),
        ];
        let mut app = test_app(workspaces, sessions);

        // Filter
        app.search_query = Some("fix".into());
        app.rebuild_tree();
        assert_eq!(app.tree.len(), 2, "workspace + 1 matching session");

        // Clear filter
        app.search_query = None;
        app.rebuild_tree();
        assert_eq!(app.tree.len(), 3, "workspace + all sessions restored");
    }

    #[test]
    fn filter_workspaces_independently() {
        let workspaces = vec![
            ws("w1", "Alpha", "/home/user/alpha"),
            ws("w2", "Beta", "/home/user/beta"),
        ];
        let sessions = vec![
            sess("s1", "fix alpha bug", "/home/user/alpha"),
            sess("s2", "fix beta bug", "/home/user/beta"),
        ];
        let mut app = test_app(workspaces, sessions);

        // Both workspaces should be present unfiltered
        assert_eq!(app.tree.len(), 4, "2 workspaces + 2 sessions");

        // Filter for "beta"
        app.search_query = Some("beta".into());
        app.rebuild_tree();

        // Should have: w1 matching (Beta in name? no) — actually w1 has name "Alpha"
        // s1 title is "fix alpha bug" — fuzzy match "beta"? No.
        // w2 name is "Beta" — matches "beta"
        // s2 title is "fix beta bug" — matches "beta"
        // So: TreeNode::Workspace(1), TreeNode::Session(1, 1)
        assert_eq!(app.tree.len(), 2, "workspace Beta + 1 matching session");
        assert!(matches!(app.tree[0], TreeNode::Workspace(1)));
        assert!(matches!(app.tree[1], TreeNode::Session(1, 1)));
    }

    // ─── agent filter tests ───

    #[test]
    fn agent_filter_shows_only_matching_sessions() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess_with_agent("s1", "claude task", "/home/user/proj", Agent::Claude),
            sess_with_agent("s2", "codex task", "/home/user/proj", Agent::Codex),
            sess_with_agent("s3", "gsd task", "/home/user/proj", Agent::Gsd),
        ];
        let mut app = test_app(workspaces, sessions);

        // Filter to Claude only
        app.agent_filter = Some(Agent::Claude);
        app.rebuild_tree();

        // Should have: Workspace(w1), Session(w1, 0) — only Claude session
        assert_eq!(app.tree.len(), 2, "workspace + 1 Claude session");
        assert!(matches!(app.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.tree[1], TreeNode::Session(0, 0))); // s1 (Claude)
    }

    #[test]
    fn agent_filter_hides_non_matching_sessions() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess_with_agent("s1", "claude task", "/home/user/proj", Agent::Claude),
            sess_with_agent("s2", "codex task", "/home/user/proj", Agent::Codex),
        ];
        let mut app = test_app(workspaces, sessions);

        // Unfiltered: workspace + 2 sessions
        assert_eq!(app.tree.len(), 3, "workspace + 2 sessions unfiltered");

        // Filter to GSD (none exist)
        app.agent_filter = Some(Agent::Gsd);
        app.rebuild_tree();

        // Without search query, workspace is still shown but has no sessions under it
        assert_eq!(
            app.tree.len(),
            1,
            "workspace header present, no matching sessions listed"
        );
        assert!(matches!(app.tree[0], TreeNode::Workspace(0)));
    }

    #[test]
    fn agent_filter_combined_with_text_search() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess_with_agent("s1", "fix bug", "/home/user/proj", Agent::Claude),
            sess_with_agent("s2", "fix bug", "/home/user/proj", Agent::Codex),
            sess_with_agent("s3", "add feature", "/home/user/proj", Agent::Claude),
            sess_with_agent("s4", "fix bug", "/home/user/proj", Agent::Gsd),
        ];
        let mut app = test_app(workspaces, sessions);

        // Filter to Claude + search "fix"
        app.agent_filter = Some(Agent::Claude);
        app.search_query = Some("fix".into());
        app.rebuild_tree();

        // Should have: Workspace(w1), Session(w1, 0) — only Claude session matching "fix"
        // s1 (Claude, "fix bug") ✓  s3 (Claude, "add feature") ✗
        assert_eq!(
            app.tree.len(),
            2,
            "workspace + 1 Claude session matching 'fix'"
        );
        assert!(matches!(app.tree[1], TreeNode::Session(0, 0))); // s1
    }

    #[test]
    fn toggle_same_agent_key_clears_filter() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess_with_agent("s1", "task a", "/home/user/proj", Agent::Claude),
            sess_with_agent("s2", "task b", "/home/user/proj", Agent::Gsd),
        ];
        let mut app = test_app(workspaces, sessions);

        // Set filter to Claude
        app.toggle_agent_filter(Agent::Claude);
        assert_eq!(app.agent_filter, Some(Agent::Claude));
        assert_eq!(app.tree.len(), 2, "workspace + 1 Claude session");

        // Toggle same agent again should clear
        app.toggle_agent_filter(Agent::Claude);
        assert_eq!(app.agent_filter, None);
        assert_eq!(app.tree.len(), 3, "workspace + all 2 sessions restored");
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
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess_with_time("s1", "old", "/home/user/proj", 100),
            sess_with_time("s2", "mid", "/home/user/proj", 500),
            sess_with_time("s3", "new", "/home/user/proj", 900),
        ];
        let mut app = test_app(workspaces, sessions);
        app.sort_mode = SortMode::TimeDesc;
        app.rebuild_tree();

        // Tree: [Workspace(0), Session(0,2), Session(0,1), Session(0,0)]
        // (newest first: s3=900, s2=500, s1=100)
        assert!(matches!(app.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.tree[1], TreeNode::Session(0, 2))); // newest
        assert!(matches!(app.tree[2], TreeNode::Session(0, 1)));
        assert!(matches!(app.tree[3], TreeNode::Session(0, 0))); // oldest
    }

    #[test]
    fn sort_time_asc_oldest_first() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess_with_time("s1", "old", "/home/user/proj", 100),
            sess_with_time("s2", "mid", "/home/user/proj", 500),
            sess_with_time("s3", "new", "/home/user/proj", 900),
        ];
        let mut app = test_app(workspaces, sessions);
        app.sort_mode = SortMode::TimeAsc;
        app.rebuild_tree();

        // Tree: [Workspace(0), Session(0,0), Session(0,1), Session(0,2)]
        // (oldest first: s1=100, s2=500, s3=900)
        assert!(matches!(app.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.tree[1], TreeNode::Session(0, 0))); // oldest
        assert!(matches!(app.tree[2], TreeNode::Session(0, 1)));
        assert!(matches!(app.tree[3], TreeNode::Session(0, 2))); // newest
    }

    #[test]
    fn sort_name_asc_alphabetical() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess("s1", "Charlie", "/home/user/proj"),
            sess("s2", "alpha", "/home/user/proj"),
            sess("s3", "Bravo", "/home/user/proj"),
        ];
        let mut app = test_app(workspaces, sessions);
        app.sort_mode = SortMode::NameAsc;
        app.rebuild_tree();

        // Case-insensitive: alpha < Bravo < Charlie
        // s2=alpha(idx1), s3=Bravo(idx2), s1=Charlie(idx0)
        assert!(matches!(app.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.tree[1], TreeNode::Session(0, 1))); // alpha
        assert!(matches!(app.tree[2], TreeNode::Session(0, 2))); // Bravo
        assert!(matches!(app.tree[3], TreeNode::Session(0, 0))); // Charlie
    }

    #[test]
    fn sort_name_desc_reverse_alphabetical() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess("s1", "Charlie", "/home/user/proj"),
            sess("s2", "alpha", "/home/user/proj"),
            sess("s3", "Bravo", "/home/user/proj"),
        ];
        let mut app = test_app(workspaces, sessions);
        app.sort_mode = SortMode::NameDesc;
        app.rebuild_tree();

        // Case-insensitive reverse: Charlie > Bravo > alpha
        // s1=Charlie(idx0), s3=Bravo(idx2), s2=alpha(idx1)
        assert!(matches!(app.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.tree[1], TreeNode::Session(0, 0))); // Charlie
        assert!(matches!(app.tree[2], TreeNode::Session(0, 2))); // Bravo
        assert!(matches!(app.tree[3], TreeNode::Session(0, 1))); // alpha
    }

    #[test]
    fn sort_agent_group_groups_by_agent() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess_with_agent("s1", "task 1", "/home/user/proj", Agent::Gsd),
            sess_with_agent("s2", "task 2", "/home/user/proj", Agent::Claude),
            sess_with_agent("s3", "task 3", "/home/user/proj", Agent::Codex),
        ];
        let mut app = test_app(workspaces, sessions);
        app.sort_mode = SortMode::AgentGroup;
        app.rebuild_tree();

        // Expected tree: Workspace(0), AgentHeader(Claude), Session(Claude),
        //                AgentHeader(Codex), Session(Codex), AgentHeader(Gsd), Session(Gsd)
        assert!(matches!(app.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.tree[1], TreeNode::AgentHeader(Agent::Claude)));
        assert!(matches!(app.tree[2], TreeNode::Session(0, 1))); // s2 = Claude
        assert!(matches!(app.tree[3], TreeNode::AgentHeader(Agent::Codex)));
        assert!(matches!(app.tree[4], TreeNode::Session(0, 2))); // s3 = Codex
        assert!(matches!(app.tree[5], TreeNode::AgentHeader(Agent::Gsd)));
        assert!(matches!(app.tree[6], TreeNode::Session(0, 0))); // s1 = Gsd
    }

    #[test]
    fn sort_agent_group_omits_empty_groups() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess_with_agent("s1", "task 1", "/home/user/proj", Agent::Claude),
            sess_with_agent("s2", "task 2", "/home/user/proj", Agent::Claude),
        ];
        let mut app = test_app(workspaces, sessions);
        app.sort_mode = SortMode::AgentGroup;
        app.rebuild_tree();

        // Only Claude sessions — no Codex or Gsd headers
        assert_eq!(app.tree.len(), 4, "workspace + header + 2 sessions");
        assert!(matches!(app.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.tree[1], TreeNode::AgentHeader(Agent::Claude)));
        assert!(matches!(app.tree[2], TreeNode::Session(0, 0)));
        assert!(matches!(app.tree[3], TreeNode::Session(0, 1)));

        // No other agent headers
        for node in &app.tree {
            if let TreeNode::AgentHeader(a) = node {
                assert_eq!(*a, Agent::Claude, "only Claude header should appear");
            }
        }
    }

    #[test]
    fn sort_with_active_filter() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess_with_time("s1", "fix alpha", "/home/user/proj", 100),
            sess_with_time("s2", "fix beta", "/home/user/proj", 500),
            sess_with_time("s3", "add feature", "/home/user/proj", 900),
        ];
        let mut app = test_app(workspaces, sessions);

        // Apply filter + sort
        app.search_query = Some("fix".into());
        app.sort_mode = SortMode::TimeAsc;
        app.rebuild_tree();

        // Filter matches s1 and s2; TimeAsc puts oldest first
        // Tree: [Workspace(0), Session(0,0) s1=100, Session(0,1) s2=500]
        assert!(matches!(app.tree[0], TreeNode::Workspace(0)));
        assert!(matches!(app.tree[1], TreeNode::Session(0, 0))); // s1 oldest
        assert!(matches!(app.tree[2], TreeNode::Session(0, 1))); // s2 newer
    }

    #[test]
    fn agent_header_is_inert_for_activate() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess_with_agent("s1", "task", "/home/user/proj", Agent::Claude),
            sess_with_agent("s2", "task", "/home/user/proj", Agent::Codex),
        ];
        let mut app = test_app(workspaces, sessions);
        app.sort_mode = SortMode::AgentGroup;
        app.rebuild_tree();

        // Find an AgentHeader node index
        let header_idx = app
            .tree
            .iter()
            .position(|n| matches!(n, TreeNode::AgentHeader(_)))
            .expect("should have an AgentHeader");
        app.tree_state.select(Some(header_idx));

        let focus_before = app.focus;
        let mode_before = app.input_mode;
        let result = app.activate_selection();
        assert!(
            result.is_ok(),
            "activate_selection on AgentHeader should succeed"
        );
        assert_eq!(app.focus, focus_before, "focus should not change");
        assert_eq!(app.input_mode, mode_before, "input_mode should not change");
    }

    #[test]
    fn agent_header_is_inert_for_delete() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess_with_agent("s1", "task", "/home/user/proj", Agent::Claude),
            sess_with_agent("s2", "task", "/home/user/proj", Agent::Codex),
        ];
        let mut app = test_app(workspaces, sessions);
        app.sort_mode = SortMode::AgentGroup;
        app.rebuild_tree();

        let session_count_before = app.sessions.len();
        let tree_len_before = app.tree.len();

        // Select an AgentHeader
        let header_idx = app
            .tree
            .iter()
            .position(|n| matches!(n, TreeNode::AgentHeader(_)))
            .expect("should have an AgentHeader");
        app.tree_state.select(Some(header_idx));
        app.delete_selected();

        // Nothing should be deleted
        assert_eq!(
            app.sessions.len(),
            session_count_before,
            "no session should be deleted"
        );
        assert_eq!(
            app.tree.len(),
            tree_len_before,
            "tree should not change after deleting AgentHeader"
        );
    }

    #[test]
    fn sort_preserves_selection_clamping() {
        let workspaces = vec![ws("w1", "Project", "/home/user/proj")];
        let sessions = vec![
            sess_with_time("s1", "old", "/home/user/proj", 100),
            sess_with_time("s2", "mid", "/home/user/proj", 500),
            sess_with_time("s3", "new", "/home/user/proj", 900),
        ];
        let mut app = test_app(workspaces, sessions);

        // Select last session (index 3 = session s3)
        app.tree_state.select(Some(3));
        assert_eq!(app.tree_state.selected(), Some(3));

        // Switch to AgentGroup mode which adds AgentHeader nodes (tree grows)
        app.sort_mode = SortMode::AgentGroup;
        app.rebuild_tree();

        // Selection should be clamped to valid range
        let sel = app.tree_state.selected();
        assert!(sel.is_some(), "selection must exist when tree non-empty");
        let idx = sel.unwrap();
        assert!(
            idx < app.tree.len(),
            "selection ({}) must be < tree len ({})",
            idx,
            app.tree.len()
        );
    }

    // ─── handle_mouse_click tests ───

    #[test]
    fn mouse_click_ignored_when_no_ptys() {
        let mut app = test_app(vec![], vec![]);
        app.tab_bar_rect = Rect::new(0, 0, 80, 1);
        // Should not panic or change state
        app.handle_mouse_click(40, 0);
        assert_eq!(app.active_pty, None);
    }

    #[test]
    fn mouse_click_ignored_when_rect_is_zero() {
        let mut app = test_app(vec![], vec![]);
        app.tab_bar_rect = Rect::default();
        app.handle_mouse_click(10, 10);
        assert_eq!(app.active_pty, None);
    }

    #[test]
    fn mouse_click_outside_tab_bar_ignored() {
        let mut app = test_app(vec![], vec![]);
        app.tab_bar_rect = Rect::new(0, 0, 80, 1);

        // Click below the tab bar (y=1 is outside a rect starting at y=0 with height=1)
        app.handle_mouse_click(40, 5);
        assert_eq!(app.active_pty, None);

        // Click above the tab bar
        app.handle_mouse_click(40, 10);
        assert_eq!(app.active_pty, None);
    }

    #[test]
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
}
