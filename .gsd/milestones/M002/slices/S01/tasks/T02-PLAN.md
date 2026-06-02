---
estimated_steps: 2
estimated_files: 2
skills_used: []
---

# T02: Fix 2 clippy warnings in discovery.rs and fmt issues

Fix 2 clippy warnings in discovery.rs and fmt issues so that cargo clippy -- -D warnings exits 0 and cargo fmt --all -- --check exits 0.

Done when: cargo clippy -- -D warnings exits 0 && cargo fmt --all -- --check exits 0.

## Inputs

- `src/discovery.rs`
- `src/app.rs`

## Expected Output

- `src/discovery.rs`
- `src/app.rs`

## Verification

cargo clippy -- -D warnings && cargo fmt --all -- --check
