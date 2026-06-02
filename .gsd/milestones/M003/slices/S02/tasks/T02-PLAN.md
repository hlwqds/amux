---
estimated_steps: 8
estimated_files: 1
skills_used: []
---

# T02: Add 1/2/3 keybindings for agent filter toggle

1. In `src/app/handler.rs`, add keybindings in the sidebar key match section:
   - `KeyCode::Char('1')` toggles `agent_filter` to/from `Some(Agent::Claude)`
   - `KeyCode::Char('2')` toggles to/from `Some(Agent::Codex)`
   - `KeyCode::Char('3')` toggles to/from `Some(Agent::Gsd)`
   - Each toggle calls `rebuild_tree()`
2. Update Esc handler in search mode to also clear `agent_filter`
3. Also handle agent filter toggle when NOT in search mode (sidebar idle)
4. Run `cargo check`

## Inputs

- `src/app/handler.rs`
- `src/app/mod.rs`

## Expected Output

- `handler.rs with agent filter toggle keys`

## Verification

cargo check
