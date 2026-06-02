---
estimated_steps: 8
estimated_files: 5
skills_used: []
---

# T02: Verify decomposition: all tests, clippy, fmt pass

Why: The decomposition must be verified as a zero-regression refactor. All 33 tests, zero clippy warnings, and fmt compliance must hold.

Do:
1. Run cargo test — all 33 tests must pass.
2. Run cargo clippy -- -D warnings — must exit 0.
3. Run cargo fmt --all -- —check — must exit 0.
4. Verify app.rs no longer exists and app/ directory contains mod.rs + 4 sub-modules.
5. If any issues found, fix them (unused imports, missing imports, fmt reformatting).

Done when: All verification commands exit 0 and file structure is confirmed.

## Inputs

- `src/app/mod.rs`
- `src/app/ui.rs`
- `src/app/handler.rs`
- `src/app/session.rs`
- `src/app/browse.rs`

## Expected Output

- `src/app/mod.rs`
- `src/app/ui.rs`
- `src/app/handler.rs`
- `src/app/session.rs`
- `src/app/browse.rs`

## Verification

cargo test
