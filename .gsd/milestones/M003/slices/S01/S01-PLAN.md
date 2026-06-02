# S01: Fuzzy search mode

**Goal:** Implement search mode infrastructure: InputMode::Search variant, code-fuzzy-match dependency, rebuild_tree filter params, slash keybinding, query typing with fuzzy filtering, search prompt rendering, and Esc exit
**Demo:** Press `/` in sidebar → type "fix" → tree filters to sessions with "fix" in title/workspace/ID → press Esc → full tree restored

## Must-Haves

- Slash key enters search mode with visible input prompt in sidebar
- Typing filters tree in real-time via fuzzy matching across session title, session ID prefix, and workspace name
- Backspace removes last character and re-filters
- Esc exits search mode, clears query, rebuilds full tree
- Empty search query shows all items (no filtering)
- Selection clamped to valid index after filter
- All existing tests pass, clippy clean

## Proof Level

- This slice proves: contract

## Integration Closure

Search mode coexists with existing PTY session management. Keystrokes in search mode are NOT forwarded to PTY. rebuild_tree is called from ~6 sites and all work correctly with or without active search.

## Verification

- Sidebar header shows [search: query] when search is active

## Tasks

- [x] **T01: Add code-fuzzy-match dependency and InputMode::Search variant** `est:5 min`
  1. Add `code-fuzzy-match = "0.2"` to Cargo.toml dependencies
  2. Add `Search` variant to `InputMode` enum in `src/types.rs`
  3. Run `cargo check` to verify compilation
  - Files: `Cargo.toml`, `src/types.rs`
  - Verify: cargo check

- [x] **T02: Add search fields to App and modify rebuild_tree with filter logic** `est:20 min`
  1. Add `search_query: Option<String>` field to App struct in `src/app/mod.rs`
  2. Initialize to `None` in `App::new()`
  3. Modify `rebuild_tree()` to read `self.search_query` and filter sessions/workspaces:
     - When query is Some(non-empty), score each session's title + short ID + workspace name against query using `code_fuzzy_match::fuzzy_match`
     - Only include sessions with a positive score
     - Only include workspaces that have at least one matching session or match the query themselves
     - When query is None or empty, show all items (no filtering)
  4. After rebuilding tree, clamp selection to valid range (call `move_sel(0)` if tree is non-empty)
  5. Run `cargo check`
  - Files: `src/app/mod.rs`
  - Verify: cargo check && cargo test

- [x] **T03: Add slash keybinding and search mode key handling** `est:15 min`
  1. In `src/app/handler.rs`, add slash keybinding in the sidebar key match section:
     - `KeyCode::Char('/')` sets `input_mode = InputMode::Search`, clears `input_buffer`
  2. Add `InputMode::Search` handling in `handle_input_key()`:
     - `KeyCode::Char(c)` pushes char to `input_buffer`, sets `search_query = Some(input_buffer.clone())`, calls `rebuild_tree()`
     - `KeyCode::Backspace` pops last char from `input_buffer`; if empty, sets `search_query = None`; calls `rebuild_tree()`
     - `KeyCode::Esc` sets `input_mode = InputMode::None`, clears `input_buffer`, sets `search_query = None`, calls `rebuild_tree()`
  3. Run `cargo check`
  - Files: `src/app/handler.rs`
  - Verify: cargo check

- [x] **T04: Render search prompt and filter indicator in sidebar** `est:10 min`
  1. In `src/app/ui.rs`, modify `render_sidebar()`:
     - When `input_mode == InputMode::Search`, render a search input line at the bottom of the sidebar area showing the prompt with current input_buffer content
     - Update the sidebar block title to show `[search: {query}]` when search is active
  2. Ensure search prompt does not overlap with tree items
  3. Run `cargo check`
  - Files: `src/app/ui.rs`
  - Verify: cargo check

- [x] **T05: Verify and test fuzzy search** `est:20 min`
  1. Run `cargo test` — all existing tests must pass
  2. Run `cargo clippy -- -D warnings` — zero warnings
  3. Run `cargo fmt --check` — clean
  4. Build and manually verify search mode works end-to-end
  5. Add unit tests for rebuild_tree filter logic:
     - Test: given sessions with known titles, fuzzy query returns expected filtered tree
     - Test: empty query shows all items
     - Test: no matches shows empty tree with no panic
     - Test: selection clamped after filter
  - Files: `src/app/mod.rs`
  - Verify: cargo test && cargo clippy -- -D warnings && cargo fmt --check

## Files Likely Touched

- Cargo.toml
- src/types.rs
- src/app/mod.rs
- src/app/handler.rs
- src/app/ui.rs
