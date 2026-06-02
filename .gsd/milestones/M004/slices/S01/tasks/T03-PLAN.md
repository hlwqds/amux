---
estimated_steps: 5
estimated_files: 1
skills_used: []
---

# T03: Add `s` keybinding to cycle sort mode

In `src/app/handler.rs`:
1. In sidebar key dispatch section, add `KeyCode::Char('s')` case.
2. Call `self.cycle_sort_mode()`.
3. Return `Ok(Action::None)` (no screen transition needed, rebuild_tree handles the tree update).
4. Place after the agent filter keys (1/2/3) and before search key (/).

## Inputs

- `src/app/handler.rs`
- `src/app/mod.rs`

## Expected Output

- `src/app/handler.rs with s key sort cycling`

## Verification

cargo check && cargo test
