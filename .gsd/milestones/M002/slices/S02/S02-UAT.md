# S02: app.rs decomposition into sub-modules — UAT

**Milestone:** M002
**Written:** 2026-06-02T15:07:24.729Z

# S02: app.rs decomposition into sub-modules — UAT

**Milestone:** M002
**Written:** 2026-06-02

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: Pure structural refactor with zero functional changes — verification is entirely compile-time and test-suite based.

## Preconditions

- Rust toolchain installed (cargo, rustc, clippy, rustfmt)
- Repository at commit with S02 changes

## Smoke Test

```bash
cargo test --workspace
```
Expected: 33 passed, 0 failed.

## Test Cases

### 1. Module structure verification

1. Verify `src/app.rs` does NOT exist: `test ! -f src/app.rs`
2. Verify all 5 sub-modules exist: `src/app/mod.rs`, `src/app/ui.rs`, `src/app/handler.rs`, `src/app/session.rs`, `src/app/browse.rs`
3. **Expected:** All 5 files present, app.rs absent.

### 2. Build and test pass

1. Run `cargo build`
2. Run `cargo test --workspace`
3. **Expected:** Build succeeds, 33 tests pass, 0 failures.

### 3. Lint compliance

1. Run `cargo clippy -- -D warnings`
2. Run `cargo fmt --all -- --check`
3. **Expected:** Both exit 0 with no output.

## Edge Cases

### Sub-module independence

1. Each sub-module file must compile as part of the app module tree (verified by cargo build)
2. No orphan code — mod.rs declares all 4 sub-modules via `mod ui; mod handler; mod session; mod browse;`

## Failure Signals

- `src/app.rs` still exists (decomposition incomplete)
- cargo test failures (regression from refactor)
- clippy warnings (code quality regression)
- Missing sub-module files (incomplete extraction)

## Not Proven By This UAT

- Runtime behavior is identical (assumed from zero functional changes and passing tests)
- Performance characteristics (not in scope for structural refactor)
- Test migration to per-module locations (S03 scope)

## Notes for Tester

- This is a pure structural refactor — no behavior changes expected.
- The tests still reside in main.rs::tests module; per-module migration is S03.
