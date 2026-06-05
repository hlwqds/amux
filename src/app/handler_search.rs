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
                    self.view.status =
                    format!("Tag filter: {}", self.view.tag_filter.as_ref().unwrap_or(&String::new()))
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
                        let i = self
                            .search_result_state
                            .selected()
                            .map_or(0, |i| (i + 1).min(len - 1));
                        self.search_result_state.select(Some(i));
                    }
                    return Ok(Action::Continue);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    let i = self
                        .search_result_state
                        .selected()
                        .map_or(0, |s| s.saturating_sub(1));
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
