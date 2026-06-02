# S01: Foundation - lib.rs split and lint fixes

**Goal:** Create src/lib.rs for the lib/bin split, fix 2 clippy warnings in discovery.rs, and fix fmt issues so that cargo test passes all 33 tests, cargo clippy -- -D warnings exits 0, and cargo fmt --all -- --check exits 0.
**Demo:** cargo test passes all 33 tests, cargo clippy -- -D warnings exits 0, cargo fmt --all -- --check exits 0, and src/lib.rs exists as the library root with main.rs using it.

## Must-Haves

- src/lib.rs exists and declares all 6 public modules\n- cargo test passes all 33 tests\n- cargo clippy -- -D warnings exits 0\n- cargo fmt --all -- --check exits 0

## Proof Level

- This slice proves: contract — verified by compilation, tests, and lint tooling

## Integration Closure

src/lib.rs is the library root exposing pub mod app. src/main.rs depends on the lib crate via use amux::app.

## Verification

- Run the task and slice verification checks for this slice.

## Tasks

- [x] **T01: Create lib.rs and rewire main.rs for lib/bin split** `est:30m`
  Create src/lib.rs with: `pub mod app; pub mod config; pub mod discovery; pub mod pty; pub mod types; pub mod util;`
  In src/main.rs, remove all `mod` declarations (mod app; mod config; mod discovery; mod pty; mod types; mod util;)
  Add `use amux::app;` at the top of main.rs
  In the #[cfg(test)] mod tests block, change:
  - `use super::config::*;` → `use amux::config::*;`
  - `use super::discovery::*;` → `use amux::discovery::*;`
  - `use super::types::*;` → `use amux::types::*;`
  - `use super::util::*;` → `use amux::util::*;`
  - Files: `src/lib.rs`, `src/main.rs`
  - Verify: cargo test

- [x] **T02: Fix 2 clippy warnings in discovery.rs and fmt issues** `est:15m`
  Fix 2 clippy warnings in discovery.rs and fmt issues so that cargo clippy -- -D warnings exits 0 and cargo fmt --all -- --check exits 0.
  - Files: `src/discovery.rs`, `src/app.rs`
  - Verify: cargo clippy -- -D warnings && cargo fmt --all -- --check

## Files Likely Touched

- src/lib.rs
- src/main.rs
- src/discovery.rs
- src/app.rs
