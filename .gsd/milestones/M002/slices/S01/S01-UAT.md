# S01: Foundation - lib.rs split and lint fixes — UAT

**Milestone:** M002
**Written:** 2026-06-02T14:38:02.857Z

# S01: Foundation - lib.rs split and lint fixes — UAT

**Milestone:** M002
**Written:** 2026-06-02

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: This is a pure structural refactor with no runtime behavior changes. Verification is fully covered by compilation, test suite, and lint tooling.

## Preconditions

- Rust toolchain installed (cargo, clippy, rustfmt)
- Project cloned and dependencies resolved (`cargo build` succeeds)

## Smoke Test

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --all -- --check
```

All three commands must exit 0.

## Test Cases

### 1. Library root exists and exposes correct modules

1. `cat src/lib.rs`
2. **Expected:** File contains `pub mod app;`, `pub mod config;`, `pub mod discovery;`, `pub mod pty;`, `pub mod types;`, `pub mod util;` — exactly 6 public module declarations.

### 2. All 33 tests pass via crate imports

1. `cargo test 2>&1`
2. **Expected:** Output shows `test result: ok. 33 passed; 0 failed`. Tests in `main.rs::tests` use `amux::` imports and resolve correctly.

### 3. Zero clippy warnings

1. `cargo clippy -- -D warnings 2>&1`
2. **Expected:** Exit code 0. No warning output.

### 4. Zero formatting issues

1. `cargo fmt --all -- --check 2>&1`
2. **Expected:** Exit code 0. No diff output.

### 5. main.rs is a thin entry point using the lib crate

1. `grep -c 'mod ' src/main.rs` — should be 0 (no mod declarations)
2. `grep 'use amux::' src/main.rs` — should show `use amux::app;`
3. **Expected:** main.rs has no `mod` declarations and imports from the `amux` crate.

## Edge Cases

### Test imports after lib split
- The `#[cfg(test)] mod tests` block in main.rs uses `use amux::config::*`, `use amux::discovery::*`, etc. These must resolve correctly against the library crate, not via `super::`.

## Failure Signals

- `cargo test` shows any failed test
- `cargo clippy -- -D warnings` exits non-zero
- `cargo fmt --all -- --check` exits non-zero
- `src/lib.rs` is missing or doesn't declare all 6 modules

## Not Proven By This UAT

- This UAT does not verify that app.rs has been decomposed into sub-modules (that's S02)
- This UAT does not verify that tests have been migrated to per-module locations (that's S03)
- No runtime/functional behavior testing — this slice is purely structural

## Notes for Tester

- The package name in Cargo.toml is `amux`, which becomes the lib crate name for imports
- No new dependencies were added
- The 2 clippy fixes were in discovery.rs only — no other source files were modified for lint purposes
