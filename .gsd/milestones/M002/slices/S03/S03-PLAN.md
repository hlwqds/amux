# S03: Test migration to per-module locations

**Goal:** Migrate all 33 tests from main.rs::tests to per-module #[cfg(test)] mod tests blocks in config.rs (8), types.rs (6), discovery.rs (18), and util.rs (1), then strip main.rs to a 5-line entry point. Zero test logic changes — purely mechanical relocation of test functions with adjusted import blocks.
**Demo:** All 33 tests relocated from main.rs::tests to per-module #[cfg(test)] mod tests blocks. main.rs is a thin entry point (10-15 lines). cargo test passes all 33 tests from their new locations.

## Must-Haves

- All 33 tests pass from per-module locations: config::tests (8), types::tests (6), discovery::tests (18), util::tests (1)
- main.rs is exactly 5 lines (use + fn main + closing brace, no test module)
- cargo clippy -- -D warnings exits 0
- cargo fmt --all -- --check exits 0
- No #[test] attributes remain in main.rs
- grep -c '#\[test\]' across the 4 target modules totals 33

## Proof Level

- This slice proves: contract — cargo test verifies all 33 tests pass from their new locations; grep counts verify distribution and main.rs cleanup

## Integration Closure

Upstream surfaces consumed: config.rs (encode_project_path), types.rs (Agent, Config, Workspace), discovery.rs (clean_user_message, extract_text_from_content, parse_codex_session, parse_gsd_session), util.rs (now_secs, relative_time). New wiring: per-module #[cfg(test)] mod tests blocks with use super::* and cross-module imports. What remains: nothing — this is the final slice in M002.

## Verification

- Run the task and slice verification checks for this slice.

## Tasks

- [x] **T01: Migrate all 33 tests to per-module locations and strip main.rs** `est:30m`
  Why: Tests currently live in a single monolithic main.rs::tests block (lines 8–454). Moving them to per-module test blocks colocates tests with the code they test, following Rust convention and completing the modular refactor.
  - Files: `src/main.rs`, `src/config.rs`, `src/types.rs`, `src/discovery.rs`, `src/util.rs`
  - Verify: cargo test --workspace

## Files Likely Touched

- src/main.rs
- src/config.rs
- src/types.rs
- src/discovery.rs
- src/util.rs
