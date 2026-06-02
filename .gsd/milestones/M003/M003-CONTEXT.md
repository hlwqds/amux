---
depends_on: [M002]
---

# M003: Search and Filter

**Gathered:** 2026-06-02
**Status:** Ready for planning

## Project Description

amux is a keyboard-first terminal UI for managing AI coding agent workspaces and sessions (Claude Code, Codex, GSD). Written in Rust (edition 2024) using ratatui + crossterm + portable-pty. After M002 completes, the codebase will be modular: `app/mod.rs` with sub-modules for UI, handler, session, and browse concerns.

## Why This Milestone

The sidebar currently displays all workspaces and sessions in a flat expandable tree with no search or filtering capability. As users accumulate multiple workspaces with dozens of sessions across three agent types, finding a specific session becomes tedious scrolling. The README advertises a `/` key for search but it was never implemented.

## User-Visible Outcome

### When this milestone is complete, the user can:

- Press `/` in sidebar mode to enter search mode with a live input prompt at the bottom of the sidebar
- Type a fuzzy query that instantly filters the tree to show only matching workspaces and sessions (matching against session title, session ID prefix, and workspace name)
- Press `1`, `2`, or `3` to toggle agent-type filters (Claude, Codex, GSD), combinable with text search
- Press `Esc` to exit search/filter mode and restore the full unfiltered tree
- See a visual indicator in the sidebar header showing active search/filter state

### Entry point / environment

- Entry point: `amux` TUI
- Environment: local terminal
- Live dependencies involved: none (pure UI feature)

## Completion Class

- Contract complete means: `/` key enters search mode, fuzzy matching filters tree in real-time, agent filter keys toggle correctly, Esc restores full tree
- Integration complete means: search/filter works alongside existing PTY session management without interference
- Operational complete means: no regressions in existing sidebar navigation, key routing, or session management

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- `/` enters search mode; typing "fix" filters to sessions with "fix" in title, case-insensitive fuzzy
- `1` toggles Claude-only filter; `2` Codex; `3` GSD; pressing same key again removes filter
- Text search + agent filter combine (e.g., "fix" + `3` shows only GSD sessions matching "fix")
- `Esc` clears all filters and restores full tree
- Existing 33+ tests pass unchanged
- `cargo clippy -- -D warnings` exits 0

## Architectural Decisions

### Fuzzy matching library

**Decision:** Add `code-fuzzy-match` crate for fuzzy scoring.

**Rationale:** Designed for command-palette use cases (VS Code-inspired). Lightweight (~2KB), returns both match score and indices for highlighting. No heavy dependencies. Alternative: hand-roll subsequence matching, but `code-fuzzy-match` is battle-tested and minimal.

**Alternatives Considered:**
- `nucleo` (helix editor's matcher) — excellent but brings SIMD complexity and a larger API surface for what we need
- `fuzzy-matcher` — uses `frizbee`/`skim` scoring, heavier dependency tree
- Hand-rolled subsequence check — avoids dependency but reinvents scoring and edge cases

### Search as InputMode variant

**Decision:** Add `InputMode::Search` variant. Reuse existing `input_buffer` for the query string. Route key events through existing `handle_input_key` dispatch.

**Rationale:** Follows established patterns (SessionName, RenameSession, etc. all use InputMode + input_buffer). Minimal structural change. Search mode is just another input mode with different rendering and tree-filtering side effects.

**Alternatives Considered:**
- Separate search state fields — would duplicate the input handling pattern unnecessarily

### Agent filter as toggle state

**Decision:** Add `agent_filter: Option<Agent>` field to App. Keys `1`/`2`/`3` toggle the filter; pressing the same key again clears it. When set, `rebuild_tree()` excludes sessions of other agent types.

**Rationale:** Simple toggle is intuitive and matches how real IDEs do it. `Option<Agent>` is the minimal representation. The filter applies as a hard exclude before fuzzy scoring, so combining it with text search is natural (fewer candidates to score).

**Alternatives Considered:**
- Multi-select agent filter (show Claude AND GSD) — over-engineered for 3 agents; toggle is simpler
- Filter as part of the search query (e.g., "claude:fix") — parsing complexity for little gain

### Filtered tree rebuilding

**Decision:** `rebuild_tree()` checks a `search_query: Option<&str>` and `agent_filter: Option<Agent>` parameter. When filters are active, it builds a subset tree containing only matching workspaces/sessions. The original unfiltered data remains in `self.sessions` and `self.workspaces`.

**Rationale:** Rebuilding the tree on each keystroke is fast (<1ms for typical session counts). No need for virtual/overlay tree or separate filtered state — just rebuild from source data with filter predicates. This avoids stale state bugs.

**Alternatives Considered:**
- Separate `filtered_tree` field — dual tree state leads to sync bugs
- Hide non-matching items via ListState skipping — fragile, breaks navigation

## Error Handling Strategy

No error handling changes. Search is a pure UI filter — failures (no matches, empty query) show as an empty or partially-filled sidebar. No I/O, no network, no fallible operations.

## Risks and Unknowns

- **Fuzzy scoring latency** — For very large session counts (>500), per-keystroke fuzzy scoring across all sessions could cause input lag. Mitigation: `code-fuzzy-match` is fast; debounce not needed for typical counts. If latency appears, add a simple `Instant`-based 50ms debounce.
- **Search mode key routing** — Must ensure `/` entry into search mode doesn't conflict with any existing sidebar keybinding. Verified: `/` is unused in sidebar mode.
- **Agent filter key conflicts** — `1`, `2`, `3` are currently unused in sidebar mode. Verified: no existing bindings.
- **Tree state on filter toggle** — When filter removes the currently-selected item, must reselect a valid index. Existing `move_sel` pattern handles this.

## Existing Codebase / Prior Art

- `src/app.rs` (→ M002: `app/mod.rs` + `app/handler.rs`, `app/ui.rs`) — handle_key dispatches sidebar keys; rebuild_tree builds the sidebar tree; render_sidebar draws it
- `InputMode` enum in `types.rs` — existing variants for input modes; Search will be added here
- `input_buffer` field on App — reused for search query
- `rebuild_tree()` method — current signature takes no args; will accept optional filter params
- `TreeNode` enum — Workspace/Session/ActiveTab variants; filtered tree uses same types
- `Agent` enum in `types.rs` — Claude/Codex/Gsd variants for agent filter
- README keybindings table — already lists `/` for search (aspirational, not implemented)

## Relevant Requirements

- New requirement: fuzzy search across session titles, workspace names, and session IDs
- New requirement: agent-type toggle filter
- No existing requirements are directly advanced by this work

## Scope

### In Scope

- Add `InputMode::Search` variant
- Add `agent_filter: Option<Agent>` field to App
- Add `code-fuzzy-match` dependency to Cargo.toml
- Implement `/` keybinding to enter search mode from sidebar
- Implement `1`/`2`/`3` agent filter toggle keys from sidebar
- Modify `rebuild_tree()` to accept optional search query and agent filter
- Implement fuzzy matching logic: score each session/workspace against query
- Render search input prompt in sidebar when in search mode
- Render filter indicator in sidebar header
- `Esc` clears search query, agent filter, and exits search mode
- Fuzzy match highlighting in rendered session/workspace text (optional, post-MVP)

### Out of Scope / Non-Goals

- Search within PTY output / scrollback
- Search history or saved searches
- Multi-select agent filter (showing multiple agent types simultaneously)
- Search across workspace content or file system
- Regex or glob-based search patterns
- Persisting search/filter state between sessions

## Technical Constraints

- Must work with M002's modular code structure (`app/` directory layout)
- `code-fuzzy-match` is the only new dependency allowed
- Must not break existing keybindings or navigation
- `edition = "2024"` Rust

## Integration Points

- `rebuild_tree()` — primary integration point for filter logic
- `handle_sidebar_key()` (M002: `app/handler.rs`) — new keybinding entries for `/`, `1`, `2`, `3`
- `render_sidebar()` (M002: `app/ui.rs`) — search prompt rendering and filter indicator
- PTY input routing — search mode must NOT forward keystrokes to PTY

## Testing Requirements

- Unit tests for fuzzy matching filter logic (given sessions + query → expected filtered set)
- Unit tests for agent filter toggle (set/clear/filter)
- Unit tests for combined text + agent filter
- Existing 33+ tests must continue passing
- Manual verification: `/` → type → see filtered results → `Esc` → full list restored

## Acceptance Criteria

1. `/` in sidebar enters search mode with visible input prompt
2. Typing in search mode filters tree in real-time via fuzzy matching
3. Fuzzy matching covers session title, session ID (short), and workspace name
4. `1`/`2`/`3` toggles agent filter; combining with text search works
5. `Esc` exits search mode, clears query and filter, restores full tree
6. Sidebar header shows active filter state (e.g., "[search: fix] [GSD]")
7. No regressions: existing tests pass, clippy clean, fmt clean
8. Binary behavior unchanged outside search mode

## Open Questions

- None — scope is well-defined and bounded
