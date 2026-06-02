---
depends_on: [M003]
---

# M004: Session Sorting and Grouping

**Gathered:** 2026-06-02
**Status:** Ready for planning

## Project Description

amux is a keyboard-first terminal UI for managing AI coding agent workspaces and sessions (Claude Code, Codex, GSD). Written in Rust (edition 2024) using ratatui + crossterm + portable-pty. The codebase is modular after M002: `app/mod.rs` with sub-modules for UI, handler, session, and browse concerns. M003 adds search/filter capability with fuzzy matching and agent-type toggle filtering.

## Why This Milestone

Sessions in the sidebar are currently sorted only by `last_active` descending (hard-coded in `discover_sessions()`). As users accumulate sessions across multiple workspaces and agents, the flat time-ordered list makes it hard to find sessions by name, see what's newest vs oldest, or view sessions grouped by agent type. After M003 adds search/filter, sorting is the natural next step for sidebar navigation.

## User-Visible Outcome

### When this milestone is complete, the user can:

- Press `s` in sidebar mode to cycle through 5 sort modes: Time ↓ → Time ↑ → Name A→Z → Name Z→A → Agent Group
- See the current sort mode displayed in the sidebar header (e.g., "[sort: name A→Z]")
- In Agent Group mode, see sessions grouped under indented agent-type sub-headers (e.g., "  ── Claude ──", "  ── GSD ──") within each workspace
- Sort applies alongside M003's search/filter — sort is applied after filter in rebuild_tree()

### Entry point / environment

- Entry point: `amux` TUI
- Environment: local terminal
- Live dependencies involved: none (pure UI feature)

## Completion Class

- Contract complete means: `s` key cycles sort modes, sessions reorder correctly in sidebar, agent group mode shows sub-headers
- Integration complete means: sort works alongside M003 search/filter without interference, existing PTY management unaffected
- Operational complete means: no regressions in existing sidebar navigation, key routing, or session management

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- `s` cycles through all 5 sort modes; sidebar header updates to show current mode
- Time ↓ matches current default behavior (sessions sorted newest-first within each workspace)
- Name A→Z sorts sessions alphabetically by title within each workspace
- Agent Group mode shows `[C] Claude` / `[X] Codex` / `[G] GSD` sub-headers with sessions grouped under them
- Sort + search/filter combine: search filters first, then sort reorders the filtered results
- Existing tests pass, `cargo clippy -- -D warnings` exits 0

## Architectural Decisions

### SortMode enum

**Decision:** Add `SortMode` enum with 5 variants: `TimeDesc`, `TimeAsc`, `NameAsc`, `NameDesc`, `AgentGroup`. Store as `sort_mode: SortMode` field on App. `s` key advances to next variant via `sort_mode.next()` method.

**Rationale:** Enum is the minimal representation. The `.next()` cycle method keeps key handling clean. Default is `TimeDesc` to preserve current behavior.

**Alternatives Considered:**
- Bitfield-based sort (direction + field) — more flexible but over-engineered for 5 modes
- Separate sort-field and sort-direction — adds UI complexity for little gain

### TreeNode::AgentHeader variant

**Decision:** Add `TreeNode::AgentHeader(Agent)` variant for agent group sub-headers. These are selectable in the tree (navigable with j/k) but inert — `activate_selection()` and `delete_selected()` treat them as no-ops.

**Rationale:** The tree is the source of truth for sidebar navigation. Header rows must be tree nodes so j/k stepping and index clamping work correctly. Making them inert (like workspace nodes don't directly "activate" to a PTY) is consistent.

**Alternatives Considered:**
- Skip-able non-node headers — would break `move_sel()` index math
- Render-only grouping without tree nodes — navigation would skip over visual boundaries, confusing

### Sort after filter in rebuild_tree()

**Decision:** In `rebuild_tree()`, apply search/filter first (as M003 does), then sort the matching sessions within each workspace before building tree nodes. This means sort operates on the already-filtered set.

**Rationale:** Filtering reduces the candidate set, making sort cheaper. Sort order shouldn't affect what gets filtered. Natural mental model: "find first, then organize results."

**Alternatives Considered:**
- Sort before filter — would sort sessions that then get hidden, wasted work
- Sort outside rebuild_tree — would require a separate sort pass and tree rebuild, doubling the work

### s key for sort cycling

**Decision:** Lowercase `s` in sidebar mode cycles through sort modes. Matches the single-key pattern of `e` (expand), `r` (refresh), etc.

**Rationale:** `s` is mnemonic for "sort". Currently unused in sidebar handler. No conflicts with M003's `/` (search), `1`/`2`/`3` (agent filter).

**Alternatives Considered:**
- `S` (shift+s) — less accessible, and shift combos might conflict with terminal emulators
- Multi-key prefix (e.g., `g s`) — over-engineered for a simple cycle

## Error Handling Strategy

No error handling changes. Sorting is a pure reordering of in-memory data. No I/O, no fallible operations. Invalid states (empty workspaces, no sessions) just result in no visible change.

## Risks and Unknowns

- **M003 merge conflict risk** — M003 is actively modifying `rebuild_tree()`, `handler.rs`, and `ui.rs`. This milestone's changes to the same files may conflict. Mitigation: M004 depends on M003 completing first; changes are additive (new field, new key case, new tree variant).
- **AgentHeader navigation edge cases** — When a workspace has no sessions of a given agent type, that agent header shouldn't appear. Also, pressing Enter or D on an AgentHeader should be a no-op, not crash. Mitigation: explicit match arms in `activate_selection()` and `delete_selected()`.
- **Sort + filter interaction** — When search filter is active and sort changes, the filtered set stays the same but reorders. Must ensure rebuild_tree applies filter first, then sort, then tree construction. Straightforward but order matters.

## Existing Codebase / Prior Art

- `src/app/mod.rs` — `rebuild_tree()` builds the sidebar tree from workspaces + sessions. M003 adds filter parameters. M004 adds sort logic after filter. Method signature will grow to accept sort_mode.
- `src/app/handler.rs` — `handle_key()` dispatches sidebar keys. M003 adds `/`, `1`/`2`/`3`. M004 adds `s` for sort cycling.
- `src/app/ui.rs` — `render_sidebar()` draws the tree. M004 adds sort mode indicator to sidebar header and renders `AgentHeader` rows with indented agent labels.
- `src/types.rs` — `TreeNode` enum gets `AgentHeader(Agent)` variant. New `SortMode` enum added here.
- `src/discovery.rs` — `discover_sessions()` currently hard-codes `Reverse(last_active)` sort. M004 does NOT change discovery — sorting is a presentation concern applied in `rebuild_tree()`.

## Relevant Requirements

- New requirement: configurable session sort order in sidebar
- New requirement: agent-type grouped view with sub-headers
- No existing requirements are directly advanced by this work

## Scope

### In Scope

- Add `SortMode` enum with 5 variants to `types.rs`
- Add `sort_mode: SortMode` field to App (default: `TimeDesc`)
- Add `TreeNode::AgentHeader(Agent)` variant to `types.rs`
- Implement `s` keybinding to cycle sort mode in sidebar
- Modify `rebuild_tree()` to apply sort after filter
- Implement sort logic for each mode (time desc/asc, name asc/desc, agent group)
- Render `AgentHeader` nodes in sidebar with indented agent label and icon
- Show current sort mode in sidebar header (e.g., "Workspaces [sort: time ↓]")
- Handle `AgentHeader` in `activate_selection()` and `delete_selected()` as no-ops
- Unit tests for sort ordering logic

### Out of Scope / Non-Goals

- Persisting sort preference between sessions (future: config file)
- Custom sort criteria or user-defined sort fields
- Sort direction reversal key (e.g., Shift+S to reverse)
- Sorting workspaces themselves (only sessions within workspaces)
- Sort within PTY output / scrollback

## Technical Constraints

- Must work with M003's search/filter (sort applies after filter in rebuild_tree)
- No new dependencies required
- Must not break existing keybindings or navigation
- `edition = "2024"` Rust

## Integration Points

- `rebuild_tree()` — primary integration: sort the session list per-workspace before building tree nodes
- `handle_key()` sidebar dispatch — new `s` key case
- `render_sidebar()` — sort mode indicator in header, `AgentHeader` rendering
- `activate_selection()` — handle `AgentHeader` as no-op
- `delete_selected()` — handle `AgentHeader` as no-op
- M003's `rebuild_tree` filter parameters — sort is applied after filter

## Testing Requirements

- Unit tests for sort mode cycling (next() wraps correctly)
- Unit tests for each sort mode producing correct session order
- Unit test for agent group mode: sessions grouped by agent with headers inserted
- Unit test for sort + filter interaction: filter first, then sort reorders
- Unit test for AgentHeader being inert (activate/delete are no-ops)
- Existing tests must continue passing
- `cargo clippy -- -D warnings` exits 0

## Acceptance Criteria

1. `s` in sidebar cycles through 5 sort modes: Time ↓ → Time ↑ → Name A→Z → Name Z→A → Agent Group
2. Sidebar header shows current sort mode indicator
3. Time ↓ is the default, matching current behavior
4. Name sorts are case-insensitive alphabetical
5. Agent Group mode shows agent-type sub-headers with sessions grouped under them
6. Sort applies after search/filter — changing sort while searching doesn't change filtered set
7. AgentHeader nodes are navigable with j/k but inert for Enter and D
8. No regressions: existing tests pass, clippy clean, fmt clean

## Open Questions

- None — scope is well-defined and bounded
