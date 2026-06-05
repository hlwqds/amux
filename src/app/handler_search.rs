use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::types::*;

impl super::App {
    pub(super) fn handle_search_key(&mut self, key: KeyEvent) -> Action {
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
        Action::Continue
    }

    pub(super) fn handle_tag_filter_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter => {
                if self.input_buffer.trim().is_empty() {
                    self.view.tag_filter = None;
                    self.view.status = "Tag filter cleared.".into();
                } else {
                    self.view.tag_filter = Some(self.input_buffer.trim().to_string());
                    self.view.status = format!(
                        "Tag filter: {}",
                        self.view.tag_filter.as_ref().unwrap_or(&String::new())
                    );
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
        Action::Continue
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

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;

    /// Build a minimal App via the test_app helper in the parent module.
    fn make_app() -> crate::app::App {
        use crate::types::*;
        let ws = crate::types::Workspace {
            id: "w1".into(),
            name: "workspace".into(),
            path: Some(std::path::PathBuf::from("/tmp/ws")),
            created_at: 1000,
            expanded: true,
        };
        let sess = crate::types::Session {
            id: "s1".into(),
            workspace_path: std::path::PathBuf::from("/tmp/ws"),
            title: "fix login bug".into(),
            last_active: 1000,
            agent: Agent::Claude,
            tags: vec!["rust".into()],
            pinned: false,
            last_message: None,
        };
        super::super::tests::test_app(vec![ws], vec![sess])
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    // ─── handle_search_key ───

    #[test]
    fn search_key_char_appends_to_query() {
        let mut app = make_app();
        app.view.input_mode = InputMode::Search;
        app.input_buffer.clear();

        app.handle_search_key(key(KeyCode::Char('f')));
        assert_eq!(app.input_buffer, "f");
        assert_eq!(app.view.search_query.as_deref(), Some("f"));

        app.handle_search_key(key(KeyCode::Char('i')));
        assert_eq!(app.input_buffer, "fi");
        assert_eq!(app.view.search_query.as_deref(), Some("fi"));
    }

    #[test]
    fn search_key_backspace_removes_last_char() {
        let mut app = make_app();
        app.view.input_mode = InputMode::Search;
        app.input_buffer = "abc".into();
        app.view.search_query = Some("abc".into());

        app.handle_search_key(key(KeyCode::Backspace));
        assert_eq!(app.input_buffer, "ab");
        assert_eq!(app.view.search_query.as_deref(), Some("ab"));
    }

    #[test]
    fn search_key_backspace_to_empty_clears_query() {
        let mut app = make_app();
        app.view.input_mode = InputMode::Search;
        app.input_buffer = "x".into();
        app.view.search_query = Some("x".into());

        app.handle_search_key(key(KeyCode::Backspace));
        assert!(app.input_buffer.is_empty());
        assert!(app.view.search_query.is_none());
    }

    #[test]
    fn search_key_esc_resets_mode_and_clears() {
        let mut app = make_app();
        app.view.input_mode = InputMode::Search;
        app.input_buffer = "query".into();
        app.view.search_query = Some("query".into());
        app.view.agent_filter = Some(Agent::Claude);

        app.handle_search_key(key(KeyCode::Esc));
        assert_eq!(app.view.input_mode, InputMode::None);
        assert!(app.input_buffer.is_empty());
        assert!(app.view.search_query.is_none());
        assert!(app.view.agent_filter.is_none());
    }

    // ─── handle_tag_filter_key ───

    #[test]
    fn tag_filter_enter_sets_filter() {
        let mut app = make_app();
        app.view.input_mode = InputMode::TagFilter;
        app.input_buffer = "  rust  ".into();

        app.handle_tag_filter_key(key(KeyCode::Enter));
        assert_eq!(app.view.tag_filter.as_deref(), Some("rust"));
        assert_eq!(app.view.input_mode, InputMode::None);
        assert!(app.input_buffer.is_empty());
        assert!(app.view.status.contains("rust"));
    }

    #[test]
    fn tag_filter_enter_empty_clears_filter() {
        let mut app = make_app();
        app.view.input_mode = InputMode::TagFilter;
        app.input_buffer = "   ".into();
        app.view.tag_filter = Some("old".into());

        app.handle_tag_filter_key(key(KeyCode::Enter));
        assert!(app.view.tag_filter.is_none());
        assert!(app.view.status.contains("cleared"));
    }

    #[test]
    fn tag_filter_esc_cancels_without_setting() {
        let mut app = make_app();
        app.view.input_mode = InputMode::TagFilter;
        app.input_buffer = "rust".into();
        app.view.status = "some status".into();

        app.handle_tag_filter_key(key(KeyCode::Esc));
        assert_eq!(app.view.input_mode, InputMode::None);
        assert!(app.input_buffer.is_empty());
        assert!(app.view.status.is_empty());
    }

    #[test]
    fn tag_filter_char_appends() {
        let mut app = make_app();
        app.view.input_mode = InputMode::TagFilter;
        app.input_buffer.clear();

        app.handle_tag_filter_key(key(KeyCode::Char('r')));
        app.handle_tag_filter_key(key(KeyCode::Char('s')));
        assert_eq!(app.input_buffer, "rs");
    }

    #[test]
    fn tag_filter_backspace_removes_char() {
        let mut app = make_app();
        app.view.input_mode = InputMode::TagFilter;
        app.input_buffer = "abc".into();

        app.handle_tag_filter_key(key(KeyCode::Backspace));
        assert_eq!(app.input_buffer, "ab");
    }

    // ─── handle_semantic_search_key (browsing results sub-state) ───

    #[test]
    fn semantic_search_esc_clears_results() {
        let mut app = make_app();
        app.view.input_mode = InputMode::SemanticSearch;
        app.search_results = vec![("s1".into(), 0.9)];
        app.search_result_state.select(Some(0));

        app.handle_semantic_search_key(key(KeyCode::Esc)).unwrap();
        assert!(app.search_results.is_empty());
        assert!(app.search_result_state.selected().is_none());
        assert_eq!(app.view.input_mode, InputMode::None);
    }

    #[test]
    fn semantic_search_j_moves_down() {
        let mut app = make_app();
        app.view.input_mode = InputMode::SemanticSearch;
        app.search_results = vec![("s1".into(), 0.9), ("s2".into(), 0.8), ("s3".into(), 0.7)];
        app.search_result_state.select(Some(0));

        app.handle_semantic_search_key(key(KeyCode::Char('j')))
            .unwrap();
        assert_eq!(app.search_result_state.selected(), Some(1));

        app.handle_semantic_search_key(key(KeyCode::Down)).unwrap();
        assert_eq!(app.search_result_state.selected(), Some(2));
    }

    #[test]
    fn semantic_search_j_clamps_at_last() {
        let mut app = make_app();
        app.view.input_mode = InputMode::SemanticSearch;
        app.search_results = vec![("s1".into(), 0.9), ("s2".into(), 0.8)];
        app.search_result_state.select(Some(1));

        app.handle_semantic_search_key(key(KeyCode::Char('j')))
            .unwrap();
        assert_eq!(app.search_result_state.selected(), Some(1));
    }

    #[test]
    fn semantic_search_k_moves_up() {
        let mut app = make_app();
        app.view.input_mode = InputMode::SemanticSearch;
        app.search_results = vec![("s1".into(), 0.9), ("s2".into(), 0.8), ("s3".into(), 0.7)];
        app.search_result_state.select(Some(2));

        app.handle_semantic_search_key(key(KeyCode::Char('k')))
            .unwrap();
        assert_eq!(app.search_result_state.selected(), Some(1));

        app.handle_semantic_search_key(key(KeyCode::Up)).unwrap();
        assert_eq!(app.search_result_state.selected(), Some(0));
    }

    #[test]
    fn semantic_search_k_clamps_at_zero() {
        let mut app = make_app();
        app.view.input_mode = InputMode::SemanticSearch;
        app.search_results = vec![("s1".into(), 0.9)];
        app.search_result_state.select(Some(0));

        app.handle_semantic_search_key(key(KeyCode::Char('k')))
            .unwrap();
        assert_eq!(app.search_result_state.selected(), Some(0));
    }

    #[test]
    fn semantic_search_enter_selects_and_navigates() {
        let mut app = make_app();
        app.view.input_mode = InputMode::SemanticSearch;
        app.search_results = vec![("s1".into(), 0.9)];
        app.search_result_state.select(Some(0));

        app.handle_semantic_search_key(key(KeyCode::Enter)).unwrap();
        assert!(app.search_results.is_empty());
        assert!(app.search_result_state.selected().is_none());
        assert_eq!(app.view.input_mode, InputMode::None);
    }

    // ─── handle_semantic_search_key (typing query sub-state) ───

    #[test]
    fn semantic_search_typing_char_appends() {
        let mut app = make_app();
        app.view.input_mode = InputMode::SemanticSearch;
        app.input_buffer.clear();
        // search_results is empty → typing sub-state

        app.handle_semantic_search_key(key(KeyCode::Char('q')))
            .unwrap();
        assert_eq!(app.input_buffer, "q");
    }

    #[test]
    fn semantic_search_typing_backspace_pops() {
        let mut app = make_app();
        app.view.input_mode = InputMode::SemanticSearch;
        app.input_buffer = "abc".into();

        app.handle_semantic_search_key(key(KeyCode::Backspace))
            .unwrap();
        assert_eq!(app.input_buffer, "ab");
    }

    #[test]
    fn semantic_search_typing_esc_cancels() {
        let mut app = make_app();
        app.view.input_mode = InputMode::SemanticSearch;
        app.input_buffer = "query".into();

        app.handle_semantic_search_key(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.view.input_mode, InputMode::None);
        assert!(app.input_buffer.is_empty());
    }
}
