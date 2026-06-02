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
    last_refresh: std::time::Instant,
    prev_states: Vec<PtyState>,
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
            last_refresh: std::time::Instant::now(),
            prev_states: Vec::new(),
        };
        app.rebuild_tree();
        if !app.tree.is_empty() {
            app.tree_state.select(Some(0));
        }
        app
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
                .filter(|(_, s)| self.ws_matches_path(wi, &s.workspace_path))
                .map(|(i, _)| i)
                .collect();

            if let Some(q) = query {
                // Fuzzy-filter sessions for this workspace
                let matching_sessions: Vec<usize> = sess_idxs
                    .into_iter()
                    .filter(|&si| {
                        let session = &self.sessions[si];
                        let short_id = &session.id[..session.id.len().min(8)];
                        session_fuzzy_score(session.title.as_str(), short_id, q)
                            || session_fuzzy_score(
                                &self.workspaces[wi].name,
                                short_id,
                                q,
                            )
                    })
                    .collect();

                // Fuzzy-filter active PTYs for this workspace
                let matching_ptys: Vec<usize> = self
                    .ptys
                    .iter()
                    .enumerate()
                    .filter(|(_pi, slot)| {
                        self.ws_matches_path(wi, &slot.info.workspace_path)
                            && slot.info.session_id.is_none()
                            && session_fuzzy_score(
                                &slot.info.title,
                                &slot.info.title,
                                q,
                            )
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
                    for &si in &matching_sessions {
                        tree.push(TreeNode::Session(wi, si));
                    }
                }
                ws_map.push(matching_sessions);
            } else {
                tree.push(TreeNode::Workspace(wi));
                if self.workspaces[wi].expanded {
                    for (pi, slot) in self.ptys.iter().enumerate() {
                        if self.ws_matches_path(wi, &slot.info.workspace_path)
                            && slot.info.session_id.is_none()
                        {
                            tree.push(TreeNode::ActiveTab(pi));
                        }
                    }
                    for &si in &sess_idxs {
                        tree.push(TreeNode::Session(wi, si));
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
            _ => {}
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
            None => {}
        }
        Ok(())
    }
}

/// Returns true if any of the `haystacks` fuzzy-matches `query` using code_fuzzy_match.
fn session_fuzzy_score(title: &str, short_id: &str, query: &str) -> bool {
    let check = |text: &str| -> bool {
        code_fuzzy_match::fuzzy_match(text, query).map_or(false, |score| score > 0)
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
                _ => {}
            }
        }
    };

    app.ptys.clear();
    restore_terminal(&mut terminal)?;
    result
}
