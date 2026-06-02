use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::types::*;
use crate::util::key_to_bytes;

impl super::App {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Result<Action> {
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
                        slot.handle
                            .scroll_page_up(self.last_chat_area.height.saturating_sub(2) as usize);
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
                code: KeyCode::Char('1'),
                ..
            } => {
                self.toggle_agent_filter(Agent::Claude);
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('2'),
                ..
            } => {
                self.toggle_agent_filter(Agent::Codex);
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('3'),
                ..
            } => {
                self.toggle_agent_filter(Agent::Gsd);
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('s'),
                ..
            } => {
                self.cycle_sort_mode();
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('/'),
                ..
            } => {
                self.input_mode = InputMode::Search;
                self.input_buffer.clear();
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
        if self.input_mode == InputMode::Search {
            return self.handle_search_key(key);
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

    fn handle_search_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                self.search_query = Some(self.input_buffer.clone());
                self.rebuild_tree();
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                if self.input_buffer.is_empty() {
                    self.search_query = None;
                } else {
                    self.search_query = Some(self.input_buffer.clone());
                }
                self.rebuild_tree();
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.search_query = None;
                self.agent_filter = None;
                self.rebuild_tree();
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

    pub(super) fn handle_paste(&mut self, text: &str) -> Result<Action> {
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
}
