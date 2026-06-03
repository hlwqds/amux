# S01: PTY tab bar with mouse switching

**Goal:** Implement the complete PTY tab bar: mouse capture lifecycle, Event::Mouse dispatch, tab bar rendering in chat area with layout split, click-to-switch coordinate mapping, and unit tests for tab index calculation and title truncation.
**Demo:** Spawn 2+ agent sessions, see tab bar with colored agent icons and titles, click a tab to switch, use Ctrl+J/K to cycle, Ctrl+Q to close a tab — tab bar updates immediately in all cases. Tab bar hidden when no PTYs active.

## Must-Haves

- Complete the planned slice outcomes.

## Verification

- Run the task and slice verification checks for this slice.

## Tasks

- [x] **T01: Enable mouse capture in terminal lifecycle** `est:5 min`
  Add `EnableMouseCapture` to `init_terminal()` execute! macro alongside `EnterAlternateScreen`. Add `DisableMouseCapture` to `restore_terminal()` execute! macro alongside `LeaveAlternateScreen`. Import from `crossterm::event`.
  - Files: `src/util.rs`
  - Verify: cargo clippy -- -D warnings 2>&1 | tail -5

- [x] **T02: Add tab_bar_rect field, handle_mouse_click, and Event::Mouse dispatch** `est:20 min`
  1. Add `tab_bar_rect: Rect` field to `App` struct in `src/app/mod.rs`, initialized to `Rect::default()` in `App::new()`.
  2. Add `handle_mouse_click(&mut self, x: u16, y: u16)` method to `App` impl:
     - Check if click is within `tab_bar_rect` bounds
     - Calculate tab width from available width and number of ptys
     - Map local_x to tab index
     - If valid and different from current `active_pty`, switch to it and reset scroll
     - Update status message
  3. Add `Event::Mouse` arm in the main event loop (around line 520):
     ```rust
     Event::Mouse(mouse) => {
         if let crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) = mouse.kind {
             app.handle_mouse_click(mouse.column, mouse.row);
         }
     }
     ```
  4. Import `Rect` from ratatui layout (already imported in app/mod.rs).
  - Files: `src/app/mod.rs`
  - Verify: cargo clippy -- -D warnings 2>&1 | tail -5

- [x] **T03: Render tab bar in chat area with layout split** `est:30 min`
  Modify `render_chat()` in `src/app/ui.rs` to:
  - Files: `src/app/ui.rs`
  - Verify: cargo clippy -- -D warnings 2>&1 | tail -5

- [x] **T04: Unit tests for tab index calculation and title truncation** `est:15 min`
  Add unit tests in `src/app/ui.rs` (or `src/app/mod.rs` test module):
  - Files: `src/app/ui.rs`, `src/app/mod.rs`
  - Verify: cargo test 2>&1 | tail -20

## Files Likely Touched

- src/util.rs
- src/app/mod.rs
- src/app/ui.rs
