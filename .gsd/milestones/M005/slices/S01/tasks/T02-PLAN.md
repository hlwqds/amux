---
estimated_steps: 16
estimated_files: 1
skills_used: []
---

# T02: Add tab_bar_rect field, handle_mouse_click, and Event::Mouse dispatch

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

## Inputs

- `src/app/mod.rs`

## Expected Output

- `src/app/mod.rs`

## Verification

cargo clippy -- -D warnings 2>&1 | tail -5
