# S02: app.rs decomposition into sub-modules

**Goal:** Decompose the 1558-line app.rs into app/mod.rs + 4 sub-modules (ui, handler, session, browse) using Rust's multiple impl blocks idiom. Zero functional changes — pure structural refactor.
**Demo:** app.rs no longer exists as a single file; replaced by app/mod.rs + app/ui.rs + app/handler.rs + app/session.rs + app/browse.rs. cargo build succeeds and cargo test passes all 33 tests.

## Must-Haves

- app.rs no longer exists; replaced by app/mod.rs + app/ui.rs + app/handler.rs + app/session.rs + app/browse.rs
- cargo build succeeds
- cargo test passes all 33 tests
- Each sub-module compiles independently within the app module tree
- No pub visibility changes needed (child modules access private App fields)

## Proof Level

- This slice proves: contract — compile and test pass, no runtime behavior change

## Integration Closure

- Upstream: src/lib.rs already declares pub mod app — no changes needed
- New wiring: app/mod.rs declares mod ui; mod handler; mod session; mod browse;
- Downstream: S03 test migration depends on stable module paths from this slice

## Verification

- None — pure structural refactor, no runtime behavior changes

## Tasks

- [x] **T01: Create app/ directory and extract sub-modules** `est:1h`
  Why: app.rs is a 1558-line God Object that must be split into focused sub-modules for maintainability.
  - Files: `src/app.rs`, `src/app/mod.rs`, `src/app/ui.rs`, `src/app/handler.rs`, `src/app/session.rs`, `src/app/browse.rs`
  - Verify: cargo build

- [x] **T02: Verify decomposition: all tests, clippy, fmt pass** `est:20m`
  Why: The decomposition must be verified as a zero-regression refactor. All 33 tests, zero clippy warnings, and fmt compliance must hold.
  - Files: `src/app/mod.rs`, `src/app/ui.rs`, `src/app/handler.rs`, `src/app/session.rs`, `src/app/browse.rs`
  - Verify: cargo test

## Files Likely Touched

- src/app.rs
- src/app/mod.rs
- src/app/ui.rs
- src/app/handler.rs
- src/app/session.rs
- src/app/browse.rs
