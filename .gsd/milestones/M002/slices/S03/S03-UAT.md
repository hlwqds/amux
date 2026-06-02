# S03: Test migration to per-module locations — UAT

**Milestone:** M002
**Written:** 2026-06-02T15:18:20.456Z

# S03: Test migration to per-module locations — UAT

**Milestone:** M002
**Written:** 2026-06-02

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: This slice is a pure structural refactor (test relocation) with no runtime behavior changes. Artifact verification (test pass/fail counts, file counts, lint compliance) fully covers correctness.

## Preconditions

- Working Rust toolchain installed
- Project cloned at `/home/huanglin/code/agent-workspace-tui`

## Smoke Test

Run `cargo test --workspace` and confirm 33 tests pass with 0 failures.

## Test Cases

### 1. Per-module test distribution

1. Run `grep -c '#\[test\]' src/config.rs src/types.rs src/discovery.rs src/util.rs`
2. **Expected:** config.rs=8, types.rs=6, discovery.rs=18, util.rs=1, total=33

### 2. main.rs is a thin entry point

1. Run `wc -l src/main.rs`
2. **Expected:** 5 lines
3. Run `grep -c '#\[test\]' src/main.rs`
4. **Expected:** 0

### 3. Lint compliance

1. Run `cargo clippy --workspace --tests -- -D warnings`
2. **Expected:** exits 0 with no warnings
3. Run `cargo fmt --all -- --check`
4. **Expected:** exits 0 with no diffs

### 4. All tests pass from new locations

1. Run `cargo test --workspace`
2. **Expected:** 33 passed, 0 failed; test names prefixed with config::tests::, types::tests::, discovery::tests::, util::tests::

## Edge Cases

### No test logic changed

1. Compare test function bodies before/after migration
2. **Expected:** identical logic in every test; only import blocks adjusted for new module context

## Failure Signals

- Any test failure in `cargo test`
- `#[test]` attributes found in main.rs
- Total test count != 33
- Clippy warnings or fmt diffs

## Not Proven By This UAT

- No functional behavior changes were made — this UAT does not prove new features, only that existing tests were correctly relocated
- Runtime behavior of the binary is unchanged (proven by S01/S02)

## Notes for Tester

- The only deviation from plan was removing an unused `PathBuf` import in discovery.rs's test module — no test logic was affected.
