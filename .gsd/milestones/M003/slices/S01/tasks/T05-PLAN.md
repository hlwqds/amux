---
estimated_steps: 9
estimated_files: 1
skills_used: []
---

# T05: Verify and test fuzzy search

1. Run `cargo test` — all existing tests must pass
2. Run `cargo clippy -- -D warnings` — zero warnings
3. Run `cargo fmt --check` — clean
4. Build and manually verify search mode works end-to-end
5. Add unit tests for rebuild_tree filter logic:
   - Test: given sessions with known titles, fuzzy query returns expected filtered tree
   - Test: empty query shows all items
   - Test: no matches shows empty tree with no panic
   - Test: selection clamped after filter

## Inputs

- `src/app/mod.rs`

## Expected Output

- `All tests passing`
- `Unit tests for filter logic`

## Verification

cargo test && cargo clippy -- -D warnings && cargo fmt --check
