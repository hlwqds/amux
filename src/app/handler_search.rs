use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::types::*;

impl super::App {
    pub(super) fn handle_search_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                self.view.search_query = Some(self.input_buffer.clone());
                self.rebuild_tree();
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                if self.input_buffer.is_empty() {
                    self.view.search_query = None;
                } else {
                    self.view.search_query = Some(self.input_buffer.clone());
                }
                self.rebuild_tree();
            }
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.view.search_query = None;
                self.view.agent_filter = None;
                self.rebuild_tree();
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    pub(super) fn handle_tag_filter_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Enter => {
                if self.input_buffer.trim().is_empty() {
                    self.view.tag_filter = None;
                    self.view.status = "Tag filter cleared.".into();
                } else {
                    self.view.tag_filter = Some(self.input_buffer.trim().to_string());
                    self.view.status = format!("Tag filter: {}", self.view.tag_filter.as_ref().unwrap());
                }
                self.view.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.rebuild_tree();
            }
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.view.status.clear();
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

    /// Enter scrollback search mode: capture all buffer rows for searching.
    pub(super) fn enter_scroll_search(&mut self) {
        let idx = match self.ptys.active_pty {
            Some(i) => i,
            None => return,
        };
        let slot = match self.ptys.ptys.get(idx) {
            Some(s) => s,
            None => return,
        };

        // Capture full buffer: save current scroll position, scroll to max, grab rows, restore
        let saved_offset = slot.handle.scrollback_offset();
        let parser = slot.handle.screen();
        let width: u16 = self.view.last_chat_area.width.saturating_sub(2);

        // Get total scrollback size by temporarily scrolling to max
        {
            let mut guard = parser.write();
            // set_scrollback clamps to available scrollback, so a large value = max
            guard.screen_mut().set_scrollback(999_999);
        }

        let rows: Vec<String> = {
            let guard = parser.read();
            guard.screen().rows(0, width.max(1)).collect()
        };

        // Restore original scroll position
        {
            let mut guard = parser.write();
            guard.screen_mut().set_scrollback(saved_offset);
        }

        self.ptys.scroll_search_rows = rows;
        self.ptys.scroll_search_query.clear();
        self.ptys.scroll_search_results.clear();
        self.ptys.scroll_search_result_idx = 0;
        self.view.input_mode = InputMode::ScrollSearch;
        self.view.status = "/ (search, Enter=jump, n/N=next/prev, Esc=cancel):".into();
    }

    /// Handle keys in scrollback search mode.
    pub(super) fn handle_scroll_search_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.ptys.scroll_search_query.clear();
                self.ptys.scroll_search_results.clear();
                self.ptys.scroll_search_rows.clear();
                self.ptys.scroll_search_result_idx = 0;
                self.view.status.clear();
            }
            KeyCode::Enter => {
                // Jump to first (or current) match
                self.run_scroll_search();
                if !self.ptys.scroll_search_results.is_empty() {
                    self.jump_to_search_match();
                } else {
                    self.view.status = "No matches found.".into();
                }
            }
            KeyCode::Char('n') => {
                // Next match
                if !self.ptys.scroll_search_results.is_empty() {
                    let total = self.ptys.scroll_search_results.len();
                    self.ptys.scroll_search_result_idx = (self.ptys.scroll_search_result_idx + 1) % total;
                    self.jump_to_search_match();
                }
            }
            KeyCode::Char('N') => {
                // Previous match
                if !self.ptys.scroll_search_results.is_empty() {
                    let total = self.ptys.scroll_search_results.len();
                    self.ptys.scroll_search_result_idx = (self.ptys.scroll_search_result_idx + total - 1) % total;
                    self.jump_to_search_match();
                }
            }
            KeyCode::Backspace => {
                self.ptys.scroll_search_query.pop();
                self.run_scroll_search();
                self.update_search_status();
            }
            KeyCode::Char(c) => {
                self.ptys.scroll_search_query.push(c);
                self.run_scroll_search();
                self.update_search_status();
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    /// Run search against captured rows, populating scroll_search_results.
    pub(super) fn run_scroll_search(&mut self) {
        self.ptys.scroll_search_results.clear();
        self.ptys.scroll_search_result_idx = 0;
        if self.ptys.scroll_search_query.is_empty() {
            return;
        }
        let query = self.ptys.scroll_search_query.to_lowercase();
        for (i, row) in self.ptys.scroll_search_rows.iter().enumerate() {
            if row.to_lowercase().contains(&query) {
                self.ptys.scroll_search_results.push(i);
            }
        }
    }

    /// Update status bar to show search match count.
    pub(super) fn update_search_status(&mut self) {
        let count = self.ptys.scroll_search_results.len();
        let query = &self.ptys.scroll_search_query;
        if query.is_empty() {
            self.view.status = "/ (type to search, Esc=cancel):".into();
        } else if count == 0 {
            self.view.status = format!("/{query} — no matches");
        } else {
            self.view.status = format!("/{query} — {count} match{} (Enter=jump, n/N=cycle)", if count > 1 { "es" } else { "" });
        }
    }

    /// Scroll the PTY to show the current search result match.
    fn jump_to_search_match(&mut self) {
        let idx = match self.ptys.active_pty {
            Some(i) => i,
            None => return,
        };
        let slot = match self.ptys.ptys.get(idx) {
            Some(s) => s,
            None => return,
        };
        let match_row = match self.ptys.scroll_search_results.get(self.ptys.scroll_search_result_idx) {
            Some(&r) => r,
            None => return,
        };

        let total_rows = self.ptys.scroll_search_rows.len();
        let screen_height = self.view.last_chat_area.height.saturating_sub(2) as usize;

        // scroll_search_rows was captured from the full buffer (scrollback + visible rows).
        // The row index is 0-based from the top of the full captured view.
        // We need to set scrollback so that match_row is visible (centered if possible).
        // The full buffer = scrollback rows + visible rows.
        // When scrollback_offset = 0, we see the last `screen_height` rows.
        // When scrollback_offset = X, we see rows starting from (total_rows - screen_height - X).
        // To show match_row, we want it in the visible window.

        // Calculate how many rows are in the non-visible scrollback area
        let scrollback_count = total_rows.saturating_sub(screen_height);

        // Center the match in the viewport
        let target_top = match_row.saturating_sub(screen_height / 3);
        // scrollback_offset = how far from the bottom to scroll up
        let desired_offset = scrollback_count.saturating_sub(target_top);
        let offset = desired_offset.min(scrollback_count);

        {
            let parser = slot.handle.screen();
            let mut guard = parser.write();
            guard.screen_mut().set_scrollback(offset);
        }

        let total = self.ptys.scroll_search_results.len();
        let current = self.ptys.scroll_search_result_idx + 1;
        self.view.status = format!(
            "/{} — match {}/{} (n/N=cycle, Esc=close)",
            self.ptys.scroll_search_query, current, total
        );
    }

    /// Handle keys in SemanticSearch mode.
    ///
    /// Two sub-states:
    /// 1. Typing query (search_results is empty): standard text input + Enter to search
    /// 2. Browsing results (search_results is non-empty): j/k navigate, Enter select, Esc cancel
    pub(super) fn handle_semantic_search_key(&mut self, key: KeyEvent) -> Result<Action> {
        // Sub-state: browsing results
        if !self.search_results.is_empty() {
            match key.code {
                KeyCode::Esc => {
                    self.search_results.clear();
                    self.search_result_state.select(None);
                    self.view.input_mode = InputMode::None;
                    self.view.status = "Search cancelled.".into();
                    return Ok(Action::Continue);
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    let len = self.search_results.len();
                    if len > 0 {
                        let i = self.search_result_state.selected().map_or(0, |i| (i + 1).min(len - 1));
                        self.search_result_state.select(Some(i));
                    }
                    return Ok(Action::Continue);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    let i = self.search_result_state.selected().map_or(0, |s| s.saturating_sub(1));
                    self.search_result_state.select(Some(i));
                    return Ok(Action::Continue);
                }
                KeyCode::Enter => {
                    // Navigate to the selected session in the sidebar
                    if let Some(idx) = self.search_result_state.selected()
                        && let Some((session_id, _score)) = self.search_results.get(idx)
                    {
                        let target = session_id.clone();
                        self.search_results.clear();
                        self.search_result_state.select(None);
                        self.view.input_mode = InputMode::None;
                        // Find and select the session in the tree
                        self.navigate_to_session(&target);
                    }
                    return Ok(Action::Continue);
                }
                _ => return Ok(Action::Continue),
            }
        }

        // Sub-state: typing query (standard text input)
        match key.code {
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.view.status = "Cancelled.".into();
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
}
