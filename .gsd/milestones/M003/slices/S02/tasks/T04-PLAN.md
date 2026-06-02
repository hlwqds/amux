---
estimated_steps: 13
estimated_files: 1
skills_used: []
---

# T04: Verify agent filter and combined search+filter

1. Run `cargo test` — all existing tests must pass
2. Run `cargo clippy -- -D warnings` — zero warnings
3. Run `cargo fmt --check` — clean
4. Build and manually verify:
   - Press 3, only GSD sessions shown
   - Type query with 3 active, intersection works
   - Press 3 again, filter cleared
   - Esc clears both
5. Add unit tests:
   - Test: agent filter alone shows only matching agent sessions
   - Test: agent filter + text search intersection
   - Test: workspace with zero matching sessions is hidden
   - Test: toggle same key clears filter

## Inputs

- `src/app/mod.rs`

## Expected Output

- `All tests passing`
- `Unit tests for agent filter logic`

## Verification

cargo test && cargo clippy -- -D warnings && cargo fmt --check
