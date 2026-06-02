---
depends_on: [M002]
---

# M003: Search and Filter — Research

**Date:** 2026-06-02

## Summary

M003 adds live fuzzy search and agent-type filtering to the sidebar tree. The feature is well-scoped: add `InputMode::Search`, route `/` to enter it, filter `rebuild_tree()` by an optional query + optional agent filter, and render a search prompt in the sidebar header. The `code-fuzzy-match` crate provides VS Code-style fuzzy scoring with no dependencies beyond the stdlib. All integration points are in three files: `handler.rs` (key routing), `ui.rs` (rendering), and `mod.rs` (rebuild_tree + App struct). The existing input mode pattern (`InputMode::*` + `input_buffer`) maps cleanly to search.

The primary risk is tree state management when filters shrink the tree — the selected item may vanish, requiring index clamping. This is already handled by the existing `move_sel` pattern but must be triggered after every filter change.

## Recommendation

**Two slices:** S01 (search infrastructure: `InputMode::Search`, `rebuild_tree` filter params, `/` keybinding, Esc exit) and S02 (agent filter toggle `1`/`2`/`3`, combined filter rendering, header indicators). S01 is the foundation — S02 builds on top of it with zero new architectural patterns. A single-slice approach is also viable given the small scope (~150 lines of new code), but two slices keep each one independently testable and reduce review surface.

## Implementation Landscape

### Key Files

- `src/types.rs` — Add `InputMode::Search` variant (line ~173). Already has `Agent` enum with `Claude`/`Codex`/`Gsd` variants used for filter targets.
- `src/app/mod.rs` — App struct: add `search_query: Option<String>` and `agent_filter: Option<Agent>` fields. Modify `rebuild_tree()` (line 153) to accept/use filter params. Modify `App::new()` to init new fields.
- `src/app/handler.rs` — Add `/` keybinding in sidebar match arm (after `Tab`). Add `1`/`2`/`3` keybindings. Add search mode key handling in `handle_input_key()` for `InputMode::Search`.
- `src/app/ui.rs` — Modify `render_sidebar()` to show search prompt at bottom and filter indicators in the title. Modify sidebar block title from `" Workspaces "` to include active filter state.
- `Cargo.toml` — Add `code-fuzzy-match = "0.2"` dependency.

### Build Order

1. **Add `code-fuzzy-match` to Cargo.toml + `InputMode::Search` to types.rs** — unblocks everything. Verify `cargo check` passes.
2. **Add search/filter fields to App struct** — `search_query: Option<String>`, `agent_filter: Option<Agent>`. Initialize in `App::new()`.
3. **Modify `rebuild_tree()`** — Add filter logic: when `search_query` is Some, score each session/workspace name against the query using `FuzzyMatcher`; when `agent_filter` is Some, exclude sessions of other agent types. Only include workspaces that have at least one matching session or match the query themselves.
4. **Add `/` keybinding in sidebar mode** — Sets `input_mode = InputMode::Search`, clears `input_buffer`. Add `InputMode::Search` handling in `handle_input_key()`: Char appends to `input_buffer` + triggers `rebuild_tree()`, Backspace pops + rebuilds, Esc clears search + agent filter + rebuilds full tree.
5. **Add agent filter keys `1`/`2`/`3`** — Toggle `agent_filter` in sidebar mode. If same agent already filtered, clear it. Trigger `rebuild_tree()`.
6. **Render search state in sidebar** — Show search prompt line at bottom of sidebar area when in Search mode. Update block title to show `[search: query]` and/or `[Agent]` indicators.
7. **Add selection clamping after filter** — After `rebuild_tree()` in search/filter paths, clamp `tree_state.select()` to valid range.

### Verification Approach

- `cargo test` — all existing tests must pass
- `cargo clippy -- -D warnings` — zero warnings
- Manual: run `amux`, press `/`, type query, observe filtered tree. Press `Esc`, verify full tree restored. Press `3`, verify only GSD sessions. Combine text + agent filter.
- Unit tests for `rebuild_tree` with filter params (construct App with known sessions, apply filter, assert tree contents)

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Fuzzy string scoring | `code-fuzzy-match` v0.2.2 | VS Code algorithm, zero non-stdlib deps, returns score+indices for highlighting, handles word boundaries and camelCase |

## Constraints

- **`rebuild_tree` is called from multiple sites** — `poll_states`, `refresh_sessions`, `activate_selection`, `delete_selected`, `toggle_expand`, `confirm_input`. All callers must work correctly with or without active filters. When filters are active and data changes (e.g., refresh), the filtered view must update correctly.
- **Search mode must not forward keystrokes to PTY** — The existing `handle_key` dispatch already routes input-mode keys to `handle_input_key()` instead of PTY, so `InputMode::Search` inherits this safety.
- **`/` is currently unused** in sidebar mode — confirmed by examining the sidebar match arms in `handler.rs`. No conflict.
- **`1`, `2`, `3` are currently unused** in sidebar mode — confirmed. No conflict.
- **`Agent` enum derives `Copy + Clone + PartialEq + Eq`** — perfect for `Option<Agent>` toggle pattern.

## Common Pitfalls

- **Stale selection after filter** — When the filter removes the currently-selected tree item, `tree_state` may point to an out-of-bounds index. Must always call `move_sel(0)` or clamp after `rebuild_tree()` in filter paths. The existing code in `poll_states` already does this for PTY removal but NOT for `rebuild_tree` in general.
- **Empty query showing nothing** — An empty `search_query` should either show everything (treat as "no filter") or show nothing. The UX expectation is: empty search = show all. Only trigger filtering when `search_query` is `Some` with non-empty content.
- **Workspace with zero matching sessions** — When agent filter is active, a workspace with no matching sessions should be hidden from the tree entirely (collapsed or omitted). This avoids empty workspace headers.
- **`rebuild_tree` borrows `self.workspaces` and `self.sessions`** — The method currently iterates both. Adding filter params as arguments (instead of reading from self fields) keeps the borrow checker happy. Or read the fields at the top and store in locals.

## Open Risks

- **Fuzzy matching on session ID prefix** — The context says to match against "session ID prefix" (first 8 chars). This is a secondary match target alongside title and workspace name. Must ensure the fuzzy matcher scores across all three fields and uses the best score. A simple approach: concatenate title + " " + short_id + " " + workspace_name as the match target.
- **Highlighting match indices** — `code-fuzzy-match` returns match indices, which could be used for bold/color highlighting in the sidebar. This is listed as "optional, post-MVP" in the context. Skip for S01/S02.
