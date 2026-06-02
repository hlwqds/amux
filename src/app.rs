use std::{
    fs, env,
    io::IsTerminal,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use tui_term::widget::PseudoTerminal;

use crate::config::{data_dir, generate_id, save_config_file, save_session_title, title_override_path};
use crate::discovery::{discover_sessions, find_session_jsonl};
use crate::pty::{PtyHandle, PtyState};
use crate::types::*;
use crate::util::*;

struct App {
    workspaces: Vec<Workspace>,
    sessions: Vec<Session>,
    tree: Vec<TreeNode>,
    ws_session_map: Vec<Vec<usize>>,
    tree_state: ListState,
    focus: Focus,
    input_mode: InputMode,
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
    ptys: Vec<PtySlot>,
    active_pty: Option<usize>,
    status: String,
    last_chat_area: Rect,
    last_refresh: std::time::Instant,
    prev_states: Vec<PtyState>,
}

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

        for (wi, _ws) in self.workspaces.iter().enumerate() {
            let sess_idxs: Vec<usize> = self
                .sessions
                .iter()
                .enumerate()
                .filter(|(_, s)| self.ws_matches_path(wi, &s.workspace_path))
                .map(|(i, _)| i)
                .collect();

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

        self.tree = tree;
        self.ws_session_map = ws_map;
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

    fn handle_key(&mut self, key: KeyEvent) -> Result<Action> {
        if self.input_mode != InputMode::None {
            return self.handle_input_key(key);
        }

        if self.focus == Focus::Chat {
            if let Some(idx) = self.active_pty {
                if key.code == KeyCode::Tab && !key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.focus = Focus::Sidebar;
                    self.refresh_sessions();
                    self.status = "Sessions refreshed.".into();
                    return Ok(Action::Continue);
                }
                if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.ptys.remove(idx);
                    self.active_pty = None;
                    self.focus = Focus::Sidebar;
                    self.refresh_sessions();
                    self.status = "Session terminated. Sessions refreshed.".into();
                    return Ok(Action::Continue);
                }
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && (key.code == KeyCode::Char('j') || key.code == KeyCode::Char('k'))
                {
                    if self.ptys.len() > 1 {
                        let cur = self.active_pty.unwrap_or(0);
                        let delta = if key.code == KeyCode::Char('j') {
                            1isize
                        } else {
                            -1
                        };
                        let next =
                            ((cur as isize + delta).rem_euclid(self.ptys.len() as isize)) as usize;
                        self.active_pty = Some(next);
                        if let Some(s) = self.ptys.get(next) {
                            s.handle.reset_scroll();
                        }
                        self.status = format!(
                            "Switched to: {} ({}/{})",
                            self.ptys[next].info.title,
                            next + 1,
                            self.ptys.len()
                        );
                    }
                    return Ok(Action::Continue);
                }
                // Scrollback: Page Up/Down
                if key.code == KeyCode::PageUp {
                    if let Some(slot) = self.ptys.get(idx) {
                        slot.handle.scroll_page_up(
                            self.last_chat_area.height.saturating_sub(2) as usize,
                        );
                    }
                    return Ok(Action::Continue);
                }
                if key.code == KeyCode::PageDown {
                    if let Some(slot) = self.ptys.get(idx) {
                        slot.handle.scroll_page_down(
                            self.last_chat_area.height.saturating_sub(2) as usize,
                        );
                    }
                    return Ok(Action::Continue);
                }
                let bytes = key_to_bytes(&key);
                if !bytes.is_empty()
                    && let Some(slot) = self.ptys.get(idx)
                {
                    slot.handle.reset_scroll();
                    slot.handle.write_input(&bytes);
                }
                return Ok(Action::Continue);
            }

            match key.code {
                KeyCode::Tab => {
                    self.focus = Focus::Sidebar;
                    self.refresh_sessions();
                }
                KeyCode::Char('q') | KeyCode::Esc => return Ok(Action::Quit),
                _ => {}
            }
            return Ok(Action::Continue);
        }

        // Sidebar mode
        match key {
            KeyEvent {
                code: KeyCode::Char('q') | KeyCode::Esc,
                ..
            } => Ok(Action::Quit),

            KeyEvent {
                code: KeyCode::Char('j') | KeyCode::Down,
                ..
            } => {
                self.move_sel(1);
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('k') | KeyCode::Up,
                ..
            } => {
                self.move_sel(-1);
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('e'),
                ..
            } => {
                self.toggle_expand();
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.refresh_sessions();
                self.status = "Sessions refreshed.".into();
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('R'),
                ..
            } => {
                self.start_rename();
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('N'),
                ..
            } => {
                self.start_new_workspace();
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('D'),
                ..
            } => {
                self.delete_selected();
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                if self.ptys.is_empty() {
                    self.status = "No active session. Press Enter to start one.".into();
                } else {
                    self.focus = Focus::Chat;
                    if self.active_pty.is_none() {
                        self.active_pty = Some(0);
                    }
                }
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                self.activate_selection()?;
                Ok(Action::Continue)
            }

            _ => Ok(Action::Continue),
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) -> Result<Action> {
        if self.input_mode == InputMode::BrowseDir {
            return self.handle_browse_key(key);
        }
        if self.input_mode == InputMode::SelectAgent {
            return self.handle_agent_key(key);
        }

        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.rename_target = None;
                self.rename_workspace_target = None;
                self.status = "Cancelled.".into();
            }
            KeyCode::Enter => {
                self.confirm_input()?;
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    fn handle_browse_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.new_workspace_name = None;
                self.status = "Cancelled.".into();
            }
            KeyCode::Enter => {
                self.browse_select();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.browse_move(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.browse_move(-1);
            }
            KeyCode::Backspace | KeyCode::Char('h') => {
                self.browse_up();
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    fn handle_agent_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.pending_session_name = None;
                self.status = "Cancelled.".into();
            }
            KeyCode::Enter => {
                self.confirm_input()?;
            }
            KeyCode::Char('c') | KeyCode::Char('C')
                if self.available_agents.contains(&Agent::Claude) =>
            {
                self.agent_state.select(Some(
                    self.available_agents
                        .iter()
                        .position(|a| *a == Agent::Claude)
                        .unwrap(),
                ));
                self.confirm_input()?;
            }
            KeyCode::Char('x') | KeyCode::Char('X')
                if self.available_agents.contains(&Agent::Codex) =>
            {
                self.agent_state.select(Some(
                    self.available_agents
                        .iter()
                        .position(|a| *a == Agent::Codex)
                        .unwrap(),
                ));
                self.confirm_input()?;
            }
            KeyCode::Char('g') | KeyCode::Char('G')
                if self.available_agents.contains(&Agent::Gsd) =>
            {
                self.agent_state.select(Some(
                    self.available_agents
                        .iter()
                        .position(|a| *a == Agent::Gsd)
                        .unwrap(),
                ));
                self.confirm_input()?;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let len = self.available_agents.len();
                if len > 0 {
                    let cur = self.agent_state.selected().unwrap_or(0).min(len - 1);
                    let next = (cur + 1) % len;
                    self.agent_state.select(Some(next));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let len = self.available_agents.len();
                if len > 0 {
                    let cur = self.agent_state.selected().unwrap_or(0).min(len - 1);
                    let prev = (cur + len - 1) % len;
                    self.agent_state.select(Some(prev));
                }
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    fn spawn_session(&mut self, agent: Agent) -> Result<()> {
        let name = self.pending_session_name.take();
        self.spawn_with_agent(agent, name)
    }

    fn spawn_with_agent(&mut self, agent: Agent, name: Option<String>) -> Result<()> {
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

    fn handle_paste(&mut self, text: &str) -> Result<Action> {
        if self.input_mode != InputMode::None {
            self.input_buffer.push_str(text);
        } else if self.focus == Focus::Chat
            && let Some(idx) = self.active_pty
            && let Some(slot) = self.ptys.get(idx)
        {
            slot.handle.write_input(text.as_bytes());
        }
        Ok(Action::Continue)
    }

    fn confirm_input(&mut self) -> Result<()> {
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

    fn start_rename(&mut self) {
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

    fn start_new_workspace(&mut self) {
        self.input_mode = InputMode::NewWorkspaceName;
        self.input_buffer.clear();
        self.new_workspace_name = None;
        self.status = "Workspace name (Esc = cancel):".into();
    }

    fn start_browse_dir(&mut self) {
        let home = PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/".into()));
        self.browse_dir = home;
        self.load_browse_entries();
        self.input_mode = InputMode::BrowseDir;
        self.status = "Select directory \u{00b7} Enter: open/select \u{00b7} Backspace: up \u{00b7} Esc: cancel".into();
    }

    fn load_browse_entries(&mut self) {
        let mut entries = Vec::new();

        entries.push(DirEntry {
            name: SELECT_CURRENT.into(),
            path: self.browse_dir.clone(),
            is_dir: true,
        });
        entries.push(DirEntry {
            name: SELECT_VIRTUAL.into(),
            path: PathBuf::new(),
            is_dir: false,
        });

        if self.browse_dir.parent().is_some() {
            entries.push(DirEntry {
                name: PARENT_DIR.into(),
                path: self.browse_dir.parent().unwrap().to_path_buf(),
                is_dir: true,
            });
        }

        if let Ok(rd) = fs::read_dir(&self.browse_dir) {
            let mut subdirs: Vec<DirEntry> = rd
                .flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') {
                        return None;
                    }
                    Some(DirEntry {
                        name,
                        path: e.path(),
                        is_dir: true,
                    })
                })
                .collect();
            subdirs.sort_by_key(|a| a.name.to_lowercase());
            entries.extend(subdirs);
        }

        self.browse_entries = entries;
        self.browse_state.select(Some(0));
    }

    fn browse_move(&mut self, delta: isize) {
        let len = self.browse_entries.len();
        if len == 0 {
            return;
        }
        let cur = self.browse_state.selected().unwrap_or(0).min(len - 1) as isize;
        self.browse_state
            .select(Some(((cur + delta).rem_euclid(len as isize)) as usize));
    }

    fn browse_select(&mut self) {
        let idx = match self.browse_state.selected() {
            Some(i) => i,
            None => return,
        };
        let entry = match self.browse_entries.get(idx) {
            Some(e) => e.clone(),
            None => return,
        };

        match entry.name.as_str() {
            SELECT_CURRENT => {
                let name = self.new_workspace_name.take().unwrap_or_default();
                let ws = Workspace {
                    id: generate_id(),
                    name,
                    path: Some(entry.path.clone()),
                    created_at: now_secs(),
                    expanded: true,
                };
                self.status = format!(
                    "Created workspace: {} \u{2192} {}",
                    ws.name,
                    ws.path.as_ref().unwrap().display()
                );
                self.workspaces.push(ws);
                self.save_config();
                self.rebuild_tree();
                self.input_mode = InputMode::None;
            }
            SELECT_VIRTUAL => {
                let name = self.new_workspace_name.take().unwrap_or_default();
                let ws = Workspace {
                    id: generate_id(),
                    name,
                    path: None,
                    created_at: now_secs(),
                    expanded: true,
                };
                self.status = format!("Created virtual workspace: {}", ws.name);
                self.workspaces.push(ws);
                self.save_config();
                self.rebuild_tree();
                self.input_mode = InputMode::None;
            }
            PARENT_DIR => {
                self.browse_dir = entry.path;
                self.load_browse_entries();
            }
            _ => {
                self.browse_dir = entry.path;
                self.load_browse_entries();
            }
        }
    }

    fn browse_up(&mut self) {
        if let Some(parent) = self.browse_dir.parent() {
            self.browse_dir = parent.to_path_buf();
            self.load_browse_entries();
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

    fn chat_size(&self) -> (u16, u16) {
        (
            self.last_chat_area.width.saturating_sub(2),
            self.last_chat_area.height.saturating_sub(2),
        )
    }

    // ─── Rendering ────────────────────────────────────────

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(3)])
            .split(area);

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(chunks[0]);

        self.render_sidebar(frame, cols[0]);
        self.last_chat_area = cols[1];
        self.render_chat(frame, cols[1]);
        self.render_status(frame, chunks[1]);

        if self.input_mode != InputMode::None {
            self.render_input_popup(frame, area);
        }
    }

    fn render_sidebar(&mut self, frame: &mut Frame, area: Rect) {
        let pty_states: Vec<(String, PtyState)> = self
            .ptys
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    s.info.session_id.clone().unwrap_or_default(),
                    self.pty_display_state(i),
                )
            })
            .collect();
        let active_tab_states: Vec<(PtyState, Agent)> = self
            .ptys
            .iter()
            .enumerate()
            .map(|(i, s)| (self.pty_display_state(i), s.info.agent))
            .collect();

        let items: Vec<ListItem> = self
            .tree
            .iter()
            .map(|node| match node {
                TreeNode::Workspace(wi) => {
                    let ws = &self.workspaces[*wi];
                    let icon = if ws.expanded { "\u{25bc}" } else { "\u{25b6}" };
                    let count = self.ws_session_map.get(*wi).map(|v| v.len()).unwrap_or(0);

                    let (binding_icon, binding_style, subtitle) = match &ws.path {
                        Some(p) => (
                            "\u{25c6}",
                            Style::default().fg(Color::Cyan),
                            format!("   {} sessions \u{00b7} {}", count, p.display()),
                        ),
                        None => (
                            "\u{25c7}",
                            Style::default().fg(Color::Yellow),
                            format!("   {} sessions \u{00b7} virtual", count),
                        ),
                    };

                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(
                                format!("{} {} ", icon, binding_icon),
                                binding_style.add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                ws.name.clone(),
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        Line::from(subtitle).style(Style::default().fg(Color::DarkGray)),
                    ])
                }
                TreeNode::Session(_wi, si) => {
                    if let Some(session) = self.sessions.get(*si) {
                        let short_id = &session.id[..8.min(session.id.len())];
                        let pty_info = pty_states.iter().find(|(sid, _)| sid == &session.id);
                        let pty_state = pty_info.map(|(_, s)| *s);

                        let agent_tag = Span::styled(
                            format!(" [{}]", session.agent.icon()),
                            Style::default().fg(session.agent.color()),
                        );

                        let (marker, state_tag) = match pty_state {
                            Some(PtyState::Running) => (
                                Span::styled("   \u{25cf} ", Style::default().fg(Color::Yellow)),
                                Span::styled(" [running]", Style::default().fg(Color::Yellow)),
                            ),
                            Some(PtyState::Completed) => (
                                Span::styled("   \u{25cf} ", Style::default().fg(Color::Green)),
                                Span::styled(" \u{2714} done", Style::default().fg(Color::Green)),
                            ),
                            None => (
                                Span::styled("   \u{25cb} ", Style::default().fg(Color::DarkGray)),
                                Span::raw(""),
                            ),
                        };
                        let mut spans = vec![
                            marker,
                            Span::styled(
                                relative_time(session.last_active),
                                Style::default().fg(Color::White),
                            ),
                            Span::styled(
                                format!(" ({})", short_id),
                                Style::default().fg(Color::DarkGray),
                            ),
                            state_tag,
                        ];
                        spans.push(agent_tag);
                        ListItem::new(vec![
                            Line::from(spans),
                            Line::from(format!("     {}", session.title))
                                .style(Style::default().fg(Color::Gray)),
                        ])
                    } else {
                        ListItem::new(Line::from("   \u{25cf} ?"))
                    }
                }
                TreeNode::ActiveTab(pi) => {
                    let title = self
                        .ptys
                        .get(*pi)
                        .map(|s| s.info.title.as_str())
                        .unwrap_or("New Session");
                    let info = active_tab_states.get(*pi);
                    let state = info.map(|(s, _)| *s).unwrap_or(PtyState::Running);
                    let agent = info.map(|(_, a)| *a).unwrap_or(Agent::Claude);
                    let (dot_color, state_text) = match state {
                        PtyState::Running => (Color::Yellow, " [running]"),
                        PtyState::Completed => (Color::Green, " \u{2714} done"),
                    };
                    let title_spans = vec![
                        Span::styled("   \u{25cf} ", Style::default().fg(dot_color)),
                        Span::styled(title, Style::default().fg(Color::White)),
                        Span::styled(state_text, Style::default().fg(Color::Green)),
                        Span::styled(
                            format!(" [{}]", agent.icon()),
                            Style::default().fg(agent.color()),
                        ),
                    ];
                    ListItem::new(vec![
                        Line::from(title_spans),
                        Line::from("     waiting for session file...")
                            .style(Style::default().fg(Color::DarkGray)),
                    ])
                }
            })
            .collect();

        let border_color = if self.focus == Focus::Sidebar {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Workspaces ")
            .border_style(Style::default().fg(border_color));

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(24, 36, 72))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("\u{203a}");

        frame.render_stateful_widget(list, area, &mut self.tree_state);
    }

    fn render_chat(&mut self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focus == Focus::Chat {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let scroll_offset = self.active_pty
            .and_then(|idx| self.ptys.get(idx))
            .map(|s| s.handle.scrollback_offset())
            .unwrap_or(0);

        let title = if let Some(idx) = self.active_pty {
            if let Some(slot) = self.ptys.get(idx) {
                let scroll_hint = if scroll_offset > 0 {
                    format!(" [↑{} PgDn:bottom]", scroll_offset)
                } else {
                    String::new()
                };
                format!(
                    " {} [{}] ({}/{}){} ",
                    slot.info.title,
                    slot.info.agent.label(),
                    idx + 1,
                    self.ptys.len(),
                    scroll_hint,
                )
            } else {
                " Agent ".into()
            }
        } else {
            " Agent ".into()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color));

        if let Some(idx) = self.active_pty
            && let Some(slot) = self.ptys.get(idx)
        {
            let inner = block.inner(area);
            slot.handle.resize((inner.width, inner.height));

            let parser = slot.handle.screen();
            let screen = parser.read().unwrap().screen().clone();
            let term = PseudoTerminal::new(&screen).block(block);
            frame.render_widget(term, area);
            return;
        }

        let lines = self.render_placeholder();
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_placeholder(&self) -> Vec<Line<'static>> {
        let mut lines: Vec<Line> = Vec::new();

        match self.selected_node() {
            Some(TreeNode::Workspace(wi)) => {
                let ws = &self.workspaces[*wi];
                lines.push(
                    Line::from(ws.name.clone()).style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                match &ws.path {
                    Some(p) => {
                        lines.push(
                            Line::from(format!("\u{25c6} {}", p.display()))
                                .style(Style::default().fg(Color::Green)),
                        );
                    }
                    None => {
                        lines.push(
                            Line::from("\u{25c7} Virtual workspace (no directory)")
                                .style(Style::default().fg(Color::Yellow)),
                        );
                    }
                }
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Press Enter to start a named Claude Code session")
                        .style(Style::default().fg(Color::Yellow)),
                );
            }
            Some(TreeNode::Session(_wi, si)) => {
                let session = &self.sessions[*si];
                lines.push(
                    Line::from(session.title.clone()).style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                lines.push(
                    Line::from(format!("ID: {}", session.id))
                        .style(Style::default().fg(Color::DarkGray)),
                );
                lines.push(Line::from(format!(
                    "Last active: {}",
                    relative_time(session.last_active)
                )));
                if self.pty_index_for_session(&session.id).is_some() {
                    lines.push(Line::from(""));
                    lines.push(
                        Line::from("This session is already running - Enter to switch to it")
                            .style(Style::default().fg(Color::Green)),
                    );
                } else {
                    lines.push(Line::from(""));
                    lines.push(
                        Line::from("Press Enter to resume this session")
                            .style(Style::default().fg(Color::Yellow)),
                    );
                }
            }
            Some(&TreeNode::ActiveTab(pi)) => {
                let title = self
                    .ptys
                    .get(pi)
                    .map(|s| s.info.title.clone())
                    .unwrap_or_else(|| "New Session".into());
                lines.push(
                    Line::from(title).style(
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                lines.push(
                    Line::from("Session is running...").style(Style::default().fg(Color::DarkGray)),
                );
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Press Enter to switch to this session")
                        .style(Style::default().fg(Color::Yellow)),
                );
            }
            None => {
                lines.push(Line::from("No selection").style(Style::default().fg(Color::DarkGray)));
            }
        }

        lines.push(Line::from(""));
        lines.push(
            Line::from("\u{2500}\u{2500} Keybindings \u{2500}\u{2500}")
                .style(Style::default().fg(Color::DarkGray)),
        );
        lines.push(Line::from("Enter        New (with name) / Resume / Switch"));
        lines.push(Line::from("e            Expand / collapse workspace"));
        lines.push(Line::from("j/k \u{2191}\u{2193}     Navigate tree"));
        lines.push(Line::from("r            Refresh sessions"));
        lines.push(Line::from("R            Rename selected session"));
        lines.push(Line::from("N            New workspace"));
        lines.push(Line::from("D            Delete workspace"));
        lines.push(Line::from("Tab          Toggle sidebar/chat"));
        lines.push(Line::from("Ctrl+J/K     Switch between active sessions"));
        lines.push(Line::from("Ctrl+Q       Kill current session"));
        lines.push(Line::from("q / Esc      Quit"));

        lines
    }

    fn render_input_popup(&mut self, frame: &mut Frame, area: Rect) {
        if self.input_mode == InputMode::BrowseDir {
            self.render_browse_popup(frame, area);
            return;
        }
        if self.input_mode == InputMode::SelectAgent {
            self.render_agent_popup(frame, area);
            return;
        }

        let popup = centered_rect(60, 20, area);
        frame.render_widget(Clear, popup);

        let (title, label) = match self.input_mode {
            InputMode::SessionName => (" New Session ", "Session name: "),
            InputMode::RenameSession => (" Rename Session ", "New name: "),
            InputMode::RenameWorkspace => (" Rename Workspace ", "New name: "),
            InputMode::NewWorkspaceName => (" New Workspace ", "Workspace name: "),
            _ => return,
        };

        let input = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(label, Style::default().fg(Color::Cyan).bold()),
                Span::styled(&self.input_buffer, Style::default().fg(Color::White)),
                Span::styled("_", Style::default().fg(Color::Gray)),
            ]),
            Line::from(""),
            Line::from("Enter to confirm \u{00b7} Esc to cancel")
                .style(Style::default().fg(Color::DarkGray)),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Yellow)),
        );

        frame.render_widget(input, popup);
    }

    fn render_agent_popup(&mut self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(50, 30, area);
        frame.render_widget(Clear, popup);

        let items: Vec<ListItem> = self
            .available_agents
            .iter()
            .map(|agent| {
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            format!(" [{}] ", agent.icon()),
                            Style::default()
                                .fg(agent.color())
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(agent.label(), Style::default().fg(Color::White)),
                    ]),
                    Line::from(format!("     {}", agent.cmd()))
                        .style(Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Select Agent ")
            .border_style(Style::default().fg(Color::Yellow));

        let inner = block.inner(popup);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)])
            .split(inner);

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(24, 36, 72))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("\u{203a}");
        frame.render_stateful_widget(list, chunks[0], &mut self.agent_state);

        let help = Line::from(" C:Claude  X:Codex  G:GSD  j/k:navigate  Enter:confirm  Esc:cancel")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(Paragraph::new(help), chunks[1]);
        frame.render_widget(block, popup);
    }

    fn render_browse_popup(&mut self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(70, 70, area);
        frame.render_widget(Clear, popup);

        let ws_name = self.new_workspace_name.as_deref().unwrap_or("?");
        let title = format!(" {} \u{2192} Select Directory ", ws_name);

        let items: Vec<ListItem> = self
            .browse_entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let is_select_current = entry.name == SELECT_CURRENT;
                let is_virtual = entry.name == SELECT_VIRTUAL;
                let is_parent = entry.name == PARENT_DIR;

                let style = if is_select_current {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else if is_virtual {
                    Style::default().fg(Color::Yellow)
                } else if is_parent {
                    Style::default().fg(Color::DarkGray)
                } else if i % 2 == 0 {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::Gray)
                };

                let display = if is_select_current {
                    format!("{} {}", entry.name, entry.path.display())
                } else if entry.is_dir && !is_parent && !is_virtual {
                    format!("  \u{25b8} {}", entry.name)
                } else {
                    format!("  {}", entry.name)
                };

                ListItem::new(Line::from(display)).style(style)
            })
            .collect();

        let path_line = Line::from(format!(" {}", self.browse_dir.display()))
            .style(Style::default().fg(Color::Cyan));

        let help_line = Line::from(" j/k:navigate  Enter:open/select  Backspace/h:up  Esc:cancel")
            .style(Style::default().fg(Color::DarkGray));

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Yellow));

        let inner = block.inner(popup);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .split(inner);

        frame.render_widget(Paragraph::new(path_line), chunks[0]);

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(24, 36, 72))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("\u{203a}");
        frame.render_stateful_widget(list, chunks[1], &mut self.browse_state);

        frame.render_widget(Paragraph::new(help_line), chunks[2]);
        frame.render_widget(block, popup);
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let active_count = self.ptys.len();
        let pty_status = if active_count > 0 {
            let current = self
                .active_pty
                .map(|i| {
                    self.ptys
                        .get(i)
                        .map(|s| s.info.title.as_str())
                        .unwrap_or("?")
                })
                .unwrap_or("none");
            Span::styled(
                format!(" [{} active: {}]", active_count, current),
                Style::default().fg(Color::Green),
            )
        } else {
            Span::raw("")
        };

        let line = Line::from(vec![
            Span::styled(self.status.clone(), Style::default().fg(Color::White)),
            pty_status,
            Span::raw("  "),
            Span::styled(
                "Enter:new/resume e:expand r:refresh R:rename N:new-ws D:del-ws Tab:toggle Ctrl+J/K:switch Ctrl+Q:kill q:quit",
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        frame.render_widget(
            Paragraph::new(line).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
            area,
        );
    }
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
