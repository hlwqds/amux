---
estimated_steps: 7
estimated_files: 1
skills_used: []
---

# T03: Add slash keybinding and search mode key handling

1. In `src/app/handler.rs`, add slash keybinding in the sidebar key match section:
   - `KeyCode::Char('/')` sets `input_mode = InputMode::Search`, clears `input_buffer`
2. Add `InputMode::Search` handling in `handle_input_key()`:
   - `KeyCode::Char(c)` pushes char to `input_buffer`, sets `search_query = Some(input_buffer.clone())`, calls `rebuild_tree()`
   - `KeyCode::Backspace` pops last char from `input_buffer`; if empty, sets `search_query = None`; calls `rebuild_tree()`
   - `KeyCode::Esc` sets `input_mode = InputMode::None`, clears `input_buffer`, sets `search_query = None`, calls `rebuild_tree()`
3. Run `cargo check`

## Inputs

- `src/app/handler.rs`
- `src/app/mod.rs`

## Expected Output

- `handler.rs with search mode key routing`

## Verification

cargo check
