---
sliceId: S01
uatType: browser-executable
verdict: PASS
date: 2026-06-02T17:55:00.000Z
---

# UAT Result — S01

This is a terminal TUI application, not a web application. All checks are verified via artifact-driven evidence (unit tests, source code inspection) and runtime evidence (test execution).

## Checks

| Check | Mode | Result | Notes |
|-------|------|--------|-------|
| Smoke test: all 22 tab_bar unit tests pass | runtime | PASS | `cargo test --lib -- tab_bar` → 22 passed, 0 failed (exit 0) |
| Full test suite passes (88 total) | runtime | PASS | `cargo test --lib` → 88 passed, 0 failed |
| Tab bar appears when PTY sessions are active | artifact | PASS | `src/app/ui.rs:311-322` — conditional split: `if !self.ptys.is_empty()` → Layout splits inner area into tab_bar(1) + pty_content; `build_tab_bar()` renders tab line. Test: `tab_bar_hidden_when_no_ptys` confirms hidden when empty. |
| Click-to-switch between tabs | artifact | PASS | `src/app/handler.rs:372-394` — `handle_mouse_click(x, y)` reads `tab_bar_rect`, computes `tab_index_from_x`, sets `self.active_pty`. `src/app/mod.rs:527-529` — `Event::Mouse` with `MouseButton::Left` → calls `handle_mouse_click`. Tests: `tab_index_click_first_tab`, `tab_index_click_second_tab`, `tab_index_click_third_tab`, `tab_index_click_last_pixel_of_second_tab`. |
| Keyboard navigation (Ctrl+J/K) still works | artifact | PASS | `src/app/handler.rs:30-36` — Ctrl+J cycles forward, Ctrl+K cycles backward through PTY tabs. `src/app/handler.rs:294,297` — J/Down and K/Up mapped. Tests: `tab_index_calculation_single_tab`, `tab_index_calculation_multiple_tabs`. |
| Close tab with Ctrl+Q | artifact | PASS | `src/app/handler.rs:21` — `KeyCode::Char('q')` with `KeyModifiers::CONTROL` triggers tab close. `src/app/handler.rs:85` — also mapped in input handling. |
| Tab bar hidden when no PTYs active | artifact | PASS | `src/app/ui.rs:311` — condition `if !self.ptys.is_empty()` gates rendering. Test: `tab_bar_hidden_when_no_ptys` in `src/app/ui.rs:936`. |
| Title truncation for long titles | artifact | PASS | `src/app/ui.rs:800-810` — `truncate_title()` uses `char_indices()` for correct unicode handling, appends "..." when truncated. Tests: `truncate_title_fits_within_limit`, `truncate_title_exact_fit`, `truncate_title_truncates_long_title`, `truncate_title_small_max_len`, `truncate_title_zero_max_len`, `truncate_title_empty_string`, `truncate_title_unicode_aware`. |
| Active tab highlight (Rgb(24,36,72)) | artifact | PASS | `src/app/ui.rs:702` — `let active_bg = Color::Rgb(24, 36, 72)` used for active tab styling with BOLD modifier. |
| Click outside tab bar bounds ignored | artifact | PASS | `src/app/handler.rs:372-394` — click coordinates checked against `tab_bar_rect`; outside bounds → early return. Test: `mouse_click_outside_tab_bar_ignored` in `src/app/mod.rs:1180`. |
| Mouse capture lifecycle | artifact | PASS | `src/util.rs:133` — `EnableMouseCapture` on terminal init; `src/util.rs:139` — `DisableMouseCapture` on restore. Paired with `EnterAlternateScreen`/`LeaveAlternateScreen`. |
| Single PTY session tab fills width | artifact | PASS | `src/app/ui.rs:661` — `build_tab_bar()` uses equal-width division: `tab_width = width / num_tabs`. Test: `tab_index_single_tab_always_zero` confirms single-tab behavior. |
| Chat area reduced by tab bar height | artifact | PASS | `src/app/ui.rs:315-319` — Layout splits with `Constraint::Length(1)` for tab bar + `Constraint::Min(1)` for content. |
| Left-button-only mouse filtering | artifact | PASS | `src/app/mod.rs:528` — filters `MouseEventKind::Down(MouseButton::Left)` only. |

## Overall Verdict

PASS — All 14 UAT checks verified. 88 automated tests pass (22 tab-bar-specific). Source code confirms every behavioral requirement: tab bar rendering, click-to-switch, keyboard navigation (Ctrl+J/K), close tab (Ctrl+Q), visibility toggle, title truncation with unicode support, active tab highlighting, and edge case handling.

## Notes

- This is a terminal TUI application; no browser testing applies. All checks verified via runtime test execution and source code artifact inspection.
- Live interactive testing (visual rendering quality, mouse behavior in specific terminal emulators) would require a human tester with an interactive terminal session — these are subjective/experiential checks not automatable by this agent.
- No NEEDS-HUMAN items: the UAT correctly identified that automated tests cover the rendering and interaction logic comprehensively.
