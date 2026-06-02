# M004 — Research

**Date:** 2026-06-02

## Summary

M004 adds session sorting and grouping to the sidebar. The change is well-scoped: a new `SortMode` enum with 5 variants, a `TreeNode::AgentHeader(Agent)` variant for group sub-headers, and sorting logic applied after filtering in `rebuild_tree()`. The codebase already has the patterns needed — `toggle_agent_filter()` shows how to add a cycle field + `rebuild_tree()` call, and `TreeNode::ActiveTab` shows how to add inert tree variants. The primary risk is merge conflict with M003, but M003 is complete on disk, so the code is stable.

The implementation touches 4 files: `types.rs` (SortMode enum + AgentHeader variant), `mod.rs` (sort_mode field + rebuild_tree sort logic), `handler.rs` (`s` key), and `ui.rs` (header indicator + AgentHeader rendering). No new dependencies. No external API calls. Pure in-memory reordering.

## Recommendation

**Single-slice milestone.** The work is tightly coupled — SortMode enum, rebuild_tree changes, keybinding, and UI rendering all depend on each other and can't be meaningfully parallelized. Build in this order: (1) SortMode enum in types.rs with `next()` and `label()` methods, (2) sort_mode field on App, (3) sort logic in rebuild_tree, (4) `s` key in handler, (5) header + AgentHeader rendering in ui, (6) tests. Total estimate: ~2-3 hours for a clean implementation with tests.

## Implementation Landscape

### Key Files

- `src/types.rs` — Add `SortMode` enum (5 variants: TimeDesc, TimeAsc, NameAsc, NameDesc, AgentGroup) with `next()` and `label()` methods. Add `TreeNode::AgentHeader(Agent)` variant. Session struct already has `title: String`, `last_active: u64`, `agent: Agent` — all fields needed for sorting.
- `src/app/mod.rs` — Add `sort_mode: SortMode` field to App (default: TimeDesc). Modify `rebuild_tree()` to sort filtered sessions within each workspace before building tree nodes. Agent group mode inserts `AgentHeader` nodes. `activate_selection()` and `delete_selected()` get new match arms for `AgentHeader` (no-ops). `toggle_agent_filter()` pattern shows exactly how to add a mode field + rebuild.
- `src/app/handler.rs` — Add `KeyCode::Char('s')` case in sidebar key dispatch (after `/` for search, before `Enter`). Calls `self.sort_mode = self.sort_mode.next(); self.rebuild_tree();`.
- `src/app/ui.rs` — Update sidebar header title string to include sort mode label. Add `TreeNode::AgentHeader(agent)` match arm in render_sidebar with indented agent label (e.g., `"  ── Claude Code ──"`).

### Build Order

1. **SortMode enum + AgentHeader variant** (types.rs) — Foundation. All other changes depend on these types.
2. **sort_mode field + rebuild_tree sort logic** (mod.rs) — Core behavior. Sort sessions per-workspace after filter.
3. **`s` keybinding** (handler.rs) — Wire up the cycling. Trivial once sort_mode exists.
4. **UI rendering** (ui.rs) — Header indicator + AgentHeader rendering. Visual feedback.
5. **Tests** — SortMode cycling, sort ordering, AgentHeader inertness, sort+filter interaction.

### Verification Approach

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

Key test scenarios:
- SortMode::next() cycles through all 5 variants and wraps
- Each sort mode produces correct session order (time desc = newest first, name asc = alphabetical)
- Agent group mode: sessions grouped under agent headers, empty groups omitted
- Sort + filter: filter reduces set, sort reorders within filtered set
- AgentHeader is inert for activate/delete
- rebuild_tree preserves selection clamping after reorder

## Constraints

- Rust edition 2024 — can use `let` chains in `if` guards (already used in codebase at mod.rs:271)
- `rebuild_tree()` is called from `toggle_agent_filter()`, `refresh_sessions()`, and `handle_search_key()` — sort must be applied in all paths
- No new crate dependencies — use standard `sort_by` closures
- `TreeNode` is used in a `Vec<TreeNode>` with `ListState` — adding a variant requires updating all match expressions on TreeNode

## Common Pitfalls

- **Stale session indices after sort** — `TreeNode::Session(wi, si)` stores absolute index into `self.sessions`. Sorting the session list would invalidate these indices. Instead, sort the *index list* (e.g., `sess_idxs`) and build tree nodes referencing the original positions. This is the same approach M003 uses.
- **AgentHeader selection state** — When sort mode changes to/from AgentGroup, the tree changes shape (headers added/removed). Must call `move_sel(0)` after rebuild to clamp selection, which rebuild_tree already does.
- **Case-insensitive name sort** — Must use `.to_lowercase()` or `.to_uppercase()` for comparison, not direct string comparison.
- **Empty agent groups** — In AgentGroup mode, only show headers for agent types that have at least one matching session. Don't show "── Claude ──" if there are no Claude sessions.

## Open Risks

- **rebuild_tree complexity growth** — The method already handles 3 modes (search active, no search with expansion, search with expansion) across workspaces, sessions, and PTY tabs. Adding sort logic increases complexity. Mitigation: extract sort into a helper method (`sort_session_indices`) to keep rebuild_tree readable.
