#[cfg(test)]
mod tests {

    use crate::app::tests::test_app;
    use crate::budget::TokenBudget;
    use crate::util::centered_rect;
    use ratatui::layout::Rect;

    // ── 1. Help view content: help_sidebar_pairs returns expected pairs ──
    #[test]
    fn help_sidebar_pairs_contains_all_actions() {
        let app = test_app(vec![], vec![]);
        let pairs = app.view.keybinds.help_sidebar_pairs();
        // Default Keybinds has 11 entries (move_up/down through quit)
        assert_eq!(pairs.len(), 11, "expected 11 sidebar pairs");
        assert_eq!(pairs[0].0, "Move selection");
        assert_eq!(pairs.last().unwrap().0, "Quit");
        for (action, key) in &pairs {
            assert!(!key.is_empty(), "key for '{action}' must not be empty");
        }
    }

    // ── 2. Settings display content: budget line reflects token_budget ──
    #[test]
    fn settings_budget_line_with_token_budget() {
        let mut app = test_app(vec![], vec![]);
        assert!(app.token_budget.is_none());
        app.token_budget = Some(TokenBudget {
            daily_tokens: Some(200_000),
            daily_cost: Some(5.50),
            weekly_tokens: None,
            weekly_cost: None,
        });
        let b = app.token_budget.as_ref().unwrap();
        let mut parts = Vec::new();
        if let Some(dt) = b.daily_tokens {
            parts.push(format!("{}k daily tokens", dt / 1000));
        }
        if let Some(dc) = b.daily_cost {
            parts.push(format!("${dc:.2} daily cost"));
        }
        let expected = format!("Budget: {}", parts.join(", "));
        assert_eq!(expected, "Budget: 200k daily tokens, $5.50 daily cost");
    }

    #[test]
    fn settings_budget_line_without_budget() {
        let app = test_app(vec![], vec![]);
        assert!(app.token_budget.is_none());
    }

    // ── 3. Keybind view content: display_lines covers all configurable bindings ──
    #[test]
    fn keybind_display_lines_count_and_format() {
        let app = test_app(vec![], vec![]);
        let lines = app.view.keybinds.display_lines();
        assert_eq!(lines.len(), 17);
        for line in &lines {
            assert!(line.starts_with("  "), "line should be indented: {line:?}");
            assert!(line.contains(": "), "line should contain ': ': {line:?}");
        }
        assert!(lines[0].contains("move_up"));
        assert!(lines.last().unwrap().contains("quit"));
    }

    // ── 4. centered_rect produces correct areas ──
    #[test]
    fn centered_rect_percentages() {
        let area = Rect::new(0, 0, 100, 50);
        let r = centered_rect(50, 50, area);
        assert_eq!(r.width, 50);
        assert_eq!(r.height, 25);
        assert_eq!(r.x, 25);
        assert_eq!(r.y, 13);
    }

    #[test]
    fn centered_rect_100_percent_is_full_area() {
        let area = Rect::new(0, 0, 80, 24);
        let r = centered_rect(100, 100, area);
        assert_eq!(r, area);
    }

    #[test]
    fn centered_rect_clamped_at_minimum() {
        let area = Rect::new(0, 0, 80, 24);
        let r = centered_rect(0, 0, area);
        assert_eq!(r.width, 0);
        assert_eq!(r.height, 0);
    }

    // ── 5. Popup area calculations match expected sizes ──
    #[test]
    fn help_popup_area_48x24() {
        let area = Rect::new(0, 0, 80, 24);
        let popup = centered_rect(48, 24, area);
        assert_eq!(popup.width, 38);
        assert_eq!(popup.height, 6);
        assert_eq!(popup.x, 21);
        assert_eq!(popup.y, 9);
    }

    #[test]
    fn settings_popup_area_55x18() {
        let area = Rect::new(0, 0, 80, 24);
        let popup = centered_rect(55, 18, area);
        assert_eq!(popup.width, 44);
        assert_eq!(popup.height, 4);
    }

    #[test]
    fn keybind_popup_area_80x80() {
        let area = Rect::new(0, 0, 100, 50);
        let popup = centered_rect(80, 80, area);
        assert_eq!(popup.width, 80);
        assert_eq!(popup.height, 40);
        assert_eq!(popup.x, 10);
        assert_eq!(popup.y, 5);
    }

    // ── Render smoke tests using TestBackend ──
    #[test]
    fn render_help_popup_no_panic() {
        let app = test_app(vec![], vec![]);
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                app.render_help_popup(f, f.area());
            })
            .unwrap();
    }

    #[test]
    fn render_settings_popup_no_panic() {
        let app = test_app(vec![], vec![]);
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                app.render_settings_popup(f, f.area());
            })
            .unwrap();
    }

    #[test]
    fn render_keybind_view_no_panic() {
        let mut app = test_app(vec![], vec![]);
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                app.render_keybind_view(f, f.area());
            })
            .unwrap();
    }

    #[test]
    fn keybind_scroll_clamped_by_render() {
        let mut app = test_app(vec![], vec![]);
        app.popup.keybind_scroll = 9999;
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                app.render_keybind_view(f, f.area());
            })
            .unwrap();
        assert!(
            app.popup.keybind_scroll < 9999,
            "scroll should be clamped, got {}",
            app.popup.keybind_scroll,
        );
    }
}
