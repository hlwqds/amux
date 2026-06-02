# M002 — Research

**Date:** 2026-06-02

## Summary

The amux codebase is a 3140-line Rust TUI application with a well-defined module structure — 7 source files, 9 dependencies, edition 2024. The core problem is `app.rs` at 1556 lines containing a single `struct App` with 24 private fields and a single `impl App` block with 37 methods plus one standalone `pub fn run()`. All other files (`types.rs`, `config.rs`, `discovery.rs`, `pty.rs`, `util.rs`) are well-scoped and won't need structural changes.

The decomposition of `app.rs` is the entire scope. The natural boundaries are clear from the method names: rendering (7 render_* methods, ~580 lines), key handling (4 handle_* methods + handle_paste, ~375 lines), session spawning (2 methods, ~75 lines), browsing (4 methods, ~120 lines), and core state management (new, poll_states, refresh_sessions, rebuild_tree, etc., ~250 lines). The `App` struct is the single God Object — all methods operate on `&mut self` (or `&self` for render helpers), so splitting into sub-modules via separate `impl App` blocks in different files is the correct Rust idiom.

There are two existing pre-existing issues: 2 clippy warnings in `discovery.rs` (not in `app.rs`), and `cargo fmt --check` fails due to formatting in `types.rs`. Both must be fixed as part of the "zero warnings" acceptance criteria. The 33 tests in `main.rs` are all unit tests that import via `use super::{config,discovery,types,util}::*` — they can be relocated to their respective modules with minimal effort since the tested functions are all `pub`.

## Recommendation

**Three slices, ordered by risk:**

1. **S01: Foundation (lib.rs + fix pre-existing issues)** — Create `src/lib.rs`, fix 2 clippy warnings in discovery.rs, fix fmt in types.rs. This is the lowest-risk slice that establishes the lib/bin split and brings the baseline to zero warnings. All 33 tests still pass from main.rs.

2. **S02: app.rs decomposition** — The big slice. Split `app.rs` into `app/mod.rs` (struct definition + core methods + `run()`) plus sub-modules: `app/ui.rs` (all render_* methods, ~580 lines), `app/handler.rs` (handle_key, handle_input_key, handle_browse_key, handle_agent_key, handle_paste, ~375 lines), `app/session.rs` (spawn_session, spawn_with_agent, confirm_input + related state, ~180 lines), `app/browse.rs` (browse_* methods, start_browse_dir, load_browse_entries, ~170 lines). Compile after each file split to catch borrow checker issues early.

3. **S03: Test migration** — Move 33 tests from `main.rs::tests` to per-module `#[cfg(test)] mod tests` blocks. Slim `main.rs` to ~10 lines. Verify all 33 pass.

This ordering front-loads the highest risk (app.rs decomposition in S02) while ensuring the foundation is solid and CI will be green before touching the big file.

## Implementation Landscape

### Key Files

- `src/app.rs` (1556 lines) — Primary target. Contains `struct App` (24 fields, all private), `impl App` with 37 methods, and `pub fn run()`. Must be split into `app/mod.rs` + sub-modules.
- `src/main.rs` (440 lines) — Currently 6 `mod` declarations + `fn main()` calling `app::run()` + 33 tests. After refactor: `mod` declarations stay (or move to lib.rs), main.rs becomes thin entry + integration test stub.
- `src/lib.rs` (new) — Library root. Declares all `pub mod` items. Enables per-module unit testing.
- `src/types.rs` (235 lines) — `Agent`, `Workspace`, `Config`, `Session`, `PtySlot`, `Focus`, `InputMode`, `DirEntry`, `Action`, `TreeNode`, `RunningInfo`, `ClaudeRecord`, `ClaudeMessage`. All types are `pub`. Needs fmt fix. 1 test (`agent_traits`).
- `src/discovery.rs` (485 lines) — `discover_sessions`, `parse_gsd_session`, `parse_codex_session`, `clean_user_message`, `extract_text_from_content`. Needs 2 clippy fixes. ~12 tests.
- `src/config.rs` (119 lines) — Config load/save, `generate_id`, `encode_project_path`. Compact. 3 tests.
- `src/util.rs` (142 lines) — `now_secs`, `relative_time`, `detect_agents`, `key_to_bytes`, terminal init/restore. 1 test (`relative_time_formatting`).
- `src/pty.rs` (163 lines) — PTY management, self-contained. No changes needed.

### Method Grouping for app.rs Decomposition

**Core (stays in `app/mod.rs`):**
- `new()` (L51, ~47 lines) — constructor
- `poll_states()` (L98, ~33 lines) — PTY state polling
- `pty_display_state()` (L131, ~8 lines) — PTY state helper
- `refresh_sessions()` (L139, ~17 lines) — session discovery
- `rebuild_tree()` (L156, ~33 lines) — tree rebuild
- `pty_index_for_session()` (L189, ~6 lines) — lookup
- `selected_node()` (L195, ~4 lines) — tree state accessor
- `move_sel()` (L199, ~10 lines) — selection movement
- `toggle_expand()` (L209, ~8 lines) — tree expand/collapse
- `activate_selection()` (L895, ~20 lines) — Enter key action
- `delete_selected()` (L828, ~40 lines) — D key action
- `save_config()` (L868, ~9 lines) — config persistence
- `workspace_cwd()` (L877, ~11 lines) — path helper
- `ws_matches_path()` (L888, ~7 lines) — path matcher
- `chat_size()` (L915, ~9 lines) — layout helper
- `run()` (L1491, ~65 lines) — public entry point
- `struct App` definition (~26 lines)
- Total: ~250 lines in mod.rs

**UI (goes to `app/ui.rs`):**
- `render()` (L924, ~23 lines) — top-level render
- `render_sidebar()` (L947, ~158 lines) — sidebar rendering
- `render_chat()` (L1105, ~61 lines) — chat pane rendering
- `render_placeholder()` (L1166, ~111 lines) — placeholder with session info
- `render_input_popup()` (L1277, ~42 lines) — input dialog
- `render_agent_popup()` (L1319, ~50 lines) — agent selection popup
- `render_browse_popup()` (L1369, ~79 lines) — directory browser popup
- `render_status()` (L1448, ~43 lines) — status bar
- Total: ~567 lines

**Handler (goes to `app/handler.rs`):**
- `handle_key()` (L217, ~175 lines) — main key dispatcher
- `handle_input_key()` (L392, ~30 lines) — input mode keys
- `handle_browse_key()` (L422, ~24 lines) — browse mode keys
- `handle_agent_key()` (L446, ~64 lines) — agent selection keys
- `handle_paste()` (L584, ~12 lines) — paste handling
- Total: ~305 lines

**Session (goes to `app/session.rs`):**
- `spawn_session()` (L510, ~5 lines) — session spawn entry
- `spawn_with_agent()` (L515, ~69 lines) — full spawn logic
- `confirm_input()` (L596, ~80 lines) — input confirmation dispatch
- `start_rename()` (L676, ~18 lines) — rename initiation
- `start_new_workspace()` (L694, ~7 lines) — workspace creation start
- Total: ~179 lines

**Browse (goes to `app/browse.rs`):**
- `start_browse_dir()` (L701, ~8 lines) — browse initiation
- `load_browse_entries()` (L709, ~46 lines) — directory listing
- `browse_move()` (L755, ~10 lines) — selection movement
- `browse_select()` (L765, ~56 lines) — selection action
- `browse_up()` (L821, ~7 lines) — navigate up
- Total: ~127 lines

### Test Migration Map

| Test Name | Target Module | Tested Function Location |
|-----------|--------------|------------------------|
| `encode_project_path_simple` | config.rs | config.rs |
| `encode_project_path_root` | config.rs | config.rs |
| `encode_project_path_relative` | config.rs | config.rs |
| `config_roundtrip` | config.rs | config.rs (uses types::Config, types::Workspace) |
| `workspace_serialization_virtual` | config.rs or types.rs | types.rs (Workspace struct) |
| `generate_id_is_unique` | config.rs | config.rs |
| `clean_user_message_normal` | discovery.rs | discovery.rs |
| `clean_user_message_escapes` | discovery.rs | discovery.rs |
| `clean_user_message_noise_prefix` | discovery.rs | discovery.rs |
| `clean_user_message_strips_whitespace` | discovery.rs | discovery.rs |
| `extract_text_from_string_content` | discovery.rs | discovery.rs |
| `extract_text_from_array_content` | discovery.rs | discovery.rs |
| `extract_text_from_array_with_non_text` | discovery.rs | discovery.rs |
| `extract_text_from_empty_array` | discovery.rs | discovery.rs |
| `extract_text_from_number` | discovery.rs | discovery.rs |
| `parse_codex_session_valid` | discovery.rs | discovery.rs |
| `parse_codex_session_invalid_json` | discovery.rs | discovery.rs |
| `parse_gsd_session_valid_with_gsd_run_title` | discovery.rs | discovery.rs |
| `parse_gsd_session_fallback_to_user_message` | discovery.rs | discovery.rs |
| `parse_gsd_session_gsd_run_takes_priority` | discovery.rs | discovery.rs |
| `parse_gsd_session_no_session_line` | discovery.rs | discovery.rs |
| `parse_gsd_session_title_truncated_to_50_chars` | discovery.rs | discovery.rs |
| `parse_gsd_session_empty_file` | discovery.rs | discovery.rs |
| `gsd_directory_name_encoding` | config.rs | config.rs (encode_project_path) |
| `encode_decode_gsd_dir_roundtrip` | config.rs | config.rs (encode_project_path) |
| `decode_gsd_dir_name_simple` | config.rs | config.rs (encode_project_path) |
| `decode_gsd_dir_name_root` | config.rs | config.rs (encode_project_path) |
| `discover_gsd_sessions_finds_by_workspace` | discovery.rs | discovery.rs + config.rs |
| `agent_traits` | types.rs | types.rs (Agent impl) |
| `relative_time_formatting` | util.rs | util.rs |
| `gsd_sessions_persist_after_pty_exit` | types.rs or app/mod.rs | tests retain logic from app.rs |
| `gsd_build_new_cmd_no_session_name` | types.rs | types.rs (Agent::build_new_cmd) |
| `gsd_build_resume_cmd_uses_sessions` | types.rs | types.rs (Agent::build_resume_cmd) |

### Build Order

1. **S01 first** — Create lib.rs + fix existing warnings. This is the foundation that unblocks everything else. Zero risk of breaking tests since no structural changes to app.rs.

2. **S02 second** — app.rs decomposition. This is the highest-risk slice (borrow checker, visibility, cross-module calls). Must compile after each sub-file extraction. The struct definition and `run()` stay in `app/mod.rs`; sub-modules add `impl App` blocks.

3. **S03 last** — Test migration. Depends on S01 (lib.rs for module visibility) and S02 (tests that reference app internals need stable module paths). Low risk — just moving `#[test]` functions.

### Verification Approach

After each slice:
- `cargo build` — must compile without errors
- `cargo test` — all 33 tests must pass
- `cargo clippy -- -D warnings` — zero warnings
- `cargo fmt --all -- --check` — passes

## Pre-existing Issues

Two issues exist in the current codebase that must be fixed as part of this milestone's "zero warnings" acceptance criteria:

1. **Clippy `op_ref` warning** — `src/discovery.rs:170`: `cwd_str == &p.to_string_lossy().as_ref()` should be `cwd_str == p.to_string_lossy().as_ref()`.

2. **Clippy `collapsible_if` warning** — `src/discovery.rs:218`: nested `if` + `if let` can be collapsed using edition 2024 `if let` chains.

3. **`cargo fmt` fails** — `src/types.rs` has formatting violations (long lines in `sessions_dir()` match arms).

## Constraints

- **Edition 2024 `if let` chains** — The codebase uses edition 2024, which supports `if let` in `if` chains (the `let` keyword in conditions). This is relevant for the clippy `collapsible_if` fix.
- **`impl App` across files** — Rust allows multiple `impl` blocks for the same type across files within the same crate, but all files must be sub-modules of the same module tree. The `app/` directory with `mod.rs` + sub-files is the standard pattern.
- **Visibility** — `struct App` is currently private (no `pub`). Sub-modules within `app/` can access it via `use super::*` or `use super::App`. The `pub fn run()` is the only public entry point. After lib.rs, `App` should be `pub(crate)` to allow test access from integration tests if needed.
- **No new dependencies** — Zero dependency additions are allowed.
- **`portable_pty::CommandBuilder`** — Used in `types.rs::Agent` methods but the import is at the top level. After lib.rs creation, `extern crate portable_pty` is implicit — just needs `use portable_pty::CommandBuilder` in types.rs.

## Common Pitfalls

- **Borrow checker across `impl` splits** — Some methods call `&mut self` methods while also reading `&self` fields (e.g., `handle_key` calls `self.refresh_sessions()` and reads `self.input_mode`). Since all methods remain in the same `impl App` type, splitting across files doesn't change borrow semantics — the compiler sees the same type. No pitfall here, but worth verifying.
- **Circular module imports** — If `app/ui.rs` needs types from `app/handler.rs` and vice versa, we'd have a circular dependency. Current code doesn't have this — render methods only read state, handler methods only mutate state. Safe to split.
- **Test `use super::*` paths** — After lib.rs, tests in `config.rs` use `super::*` which includes `types::*` via the `use crate::types::Config` import already in config.rs. The test for `config_roundtrip` creates `Config` and `Workspace` — these are in `types.rs` and accessed via `use crate::types::*` which config.rs already has.
- **`workspace_serialization_virtual` test** — Tests `Workspace` struct from `types.rs` but uses `serde_json`. After migration, this test should go to `types.rs` since it tests `Workspace` serialization, not config logic.

## Open Risks

- **Hidden state coupling in `confirm_input()`** — This 80-line method dispatches on all 7 `InputMode` variants and touches many fields. It could belong in `handler.rs` or `session.rs`. Placing it in `session.rs` since it orchestrates session creation flow is the cleaner choice, but it also handles workspace rename and browse — cross-cutting. May need to stay in `mod.rs` as the central dispatcher.
