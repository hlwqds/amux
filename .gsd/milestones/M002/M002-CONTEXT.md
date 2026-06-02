# M002: Modular Refactor

**Gathered:** 2026-06-02
**Status:** Ready for planning

## Project Description

amux is a keyboard-first terminal UI for managing AI coding agent workspaces and sessions (Claude Code, Codex, GSD). Written in Rust (edition 2024) using ratatui + crossterm + portable-pty. Single binary, 3140 LOC across 7 source files.

## Why This Milestone

`app.rs` contains 1556 lines — half the entire codebase — in a single `impl App` block. Rendering, event handling, session management, directory browsing, and UI popups are all interleaved. This makes the file hard to navigate, risky to modify, and blocks future feature work. The project also lacks a `lib.rs`, meaning all tests live in `main.rs` and no module-level test isolation exists.

## User-Visible Outcome

### When this milestone is complete, the user can:

- Run `cargo test` and see the same 33 tests pass (no behavior change)
- Build and use amux exactly as before — zero functional difference

### Entry point / environment

- Entry point: `cargo build` / `cargo test`
- Environment: local dev / CI
- Live dependencies involved: none (pure internal refactor)

## Completion Class

- Contract complete means: all 33 existing tests pass with identical behavior, `cargo clippy` reports zero warnings, `cargo fmt --check` passes
- Integration complete means: `cargo build --release` produces a working binary identical in behavior to pre-refactor
- Operational complete means: CI pipeline (fmt + clippy + build + test) passes green on all 4 checks

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- `cargo test` passes all 33 tests (same test names, same assertions)
- `cargo clippy -- -D warnings` exits 0
- `cargo fmt --all -- --check` exits 0
- No new dependencies added
- Binary behavior unchanged (no functional regressions)

## Architectural Decisions

### lib + bin split

**Decision:** Extract `src/lib.rs` as the library root; `src/main.rs` becomes a thin entry point calling `app::run()`.

**Rationale:** Enables per-module unit testing, allows future integration tests to import library types directly, and is the standard Rust project layout for anything beyond a toy binary.

**Alternatives Considered:**
- Keep bin-only — simpler but prevents module-level test isolation and blocks future crate reuse

### app.rs decomposition into sub-modules

**Decision:** Split `app.rs` into `app/mod.rs` (App struct, new(), run()), plus sub-modules organized by responsibility: `app/ui.rs` (rendering), `app/handler.rs` (key event handling), `app/session.rs` (spawn/PTY management), `app/browse.rs` (directory browser).

**Rationale:** Each concern is 100-400 lines, matching the one-module-per-responsibility convention. Sub-modules share access to App via `impl App` blocks in their own files — Rust allows multiple impl blocks across files within the same crate.

**Alternatives Considered:**
- Trait-based decomposition (e.g., Renderable trait) — over-engineered for a single-app struct; adds indirection without clear benefit
- Keep app.rs whole but add section comments — doesn't solve the navigation or maintenance problem

### Test migration to per-module

**Decision:** Move tests from `main.rs::tests` into `#[cfg(test)] mod tests` blocks within their respective modules (types.rs, discovery.rs, util.rs, config.rs).

**Rationale:** Tests naturally belong next to the code they test. Per-module tests can use `super::*` to access private items. This is idiomatic Rust.

**Alternatives Considered:**
- Separate `tests/` integration test directory — overkill for unit tests that need access to private functions
- Keep all tests in main.rs — current state, poor organization

## Error Handling Strategy

No error handling changes. Existing `anyhow::Result` propagation and `unwrap_or_else` patterns remain unchanged. The refactoring must preserve all error paths exactly.

## Risks and Unknowns

- **Borrow checker friction** — Moving methods out of a single `impl App` into separate files may surface borrow conflicts if methods share mutable state in non-obvious ways. Mitigation: compile after each file split.
- **Test visibility** — Some tests in main.rs test functions from multiple modules. Need to verify each test can access what it needs after migration.
- **impl block splitting** — Rust allows multiple `impl` blocks for the same type across files in the same crate, but all files must be in the same module tree under `src/`. Verify this works with the `app/` directory structure.

## Existing Codebase / Prior Art

- `src/app.rs` — 1556 lines, the primary refactoring target. Single `impl App` with 40+ methods.
- `src/main.rs` — 440 lines, 33 tests in `mod tests`, thin runtime (just calls `app::run()`).
- `src/types.rs` — 235 lines, Agent enum + data types. Well-structured, minimal changes needed.
- `src/discovery.rs` — 485 lines, session discovery per agent. May need minor visibility adjustments.
- `src/config.rs` — 119 lines, config load/save. Compact, likely unchanged.
- `src/pty.rs` — 163 lines, PTY management. Self-contained, likely unchanged.
- `src/util.rs` — 142 lines, path encoding helpers. Likely unchanged.
- `Cargo.toml` — edition 2024, 9 dependencies. No changes needed.
- `.github/workflows/ci.yml` — fmt + clippy + build + test. No changes needed.

## Relevant Requirements

- R009 — Unit tests for GSD session parsing (tests must remain passing after migration)

## Scope

### In Scope

- Split `app.rs` into `app/mod.rs` + sub-modules (ui, handler, session, browse)
- Create `src/lib.rs` as library root
- Slim `src/main.rs` to entry point + integration tests only
- Migrate 33 unit tests to per-module `#[cfg(test)]` blocks
- Fix 2 existing clippy warnings
- Ensure `cargo fmt --check` passes

### Out of Scope / Non-Goals

- Adding new features or changing any user-visible behavior
- Adding new dependencies
- Changing the CI pipeline
- Introducing trait-based abstractions or design patterns not already present
- Refactoring discovery.rs, pty.rs, config.rs, or util.rs (unless test migration requires visibility tweaks)

## Technical Constraints

- Must compile with `--edition 2024`
- Zero new dependency additions
- All 33 existing test assertions must pass without modification to test logic
- No functional behavior changes — this is a pure structural refactor

## Integration Points

- CI pipeline (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo build`, `cargo test`) — must pass identically post-refactor
- No external system integration changes

## Testing Requirements

All 33 existing unit tests must pass after migration. Tests should be relocated to their respective modules:
- Agent/types tests → `types.rs`
- Discovery/encoding tests → `discovery.rs`
- Config serialization tests → `config.rs`
- Util/encoding tests → `util.rs`

No new tests required, but existing tests must compile and pass from their new locations.

## Acceptance Criteria

1. `app.rs` no longer exists as a single file — replaced by `app/mod.rs` + sub-modules
2. `src/lib.rs` exists and exposes the public API
3. `src/main.rs` is ≤50 lines (entry point only)
4. All 33 tests pass from per-module locations
5. `cargo clippy -- -D warnings` exits 0 (zero warnings)
6. `cargo fmt --all -- --check` exits 0
7. `cargo build --release` produces a working binary

## Open Questions

- None — scope is well-defined and bounded
