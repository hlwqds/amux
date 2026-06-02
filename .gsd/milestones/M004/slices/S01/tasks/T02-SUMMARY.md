---
id: T02
parent: S01
milestone: M004
key_files:
  - src/app/mod.rs
  - src/types.rs
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-02T17:01:11.983Z
blocker_discovered: false
---

# T02: Added sort_mode field to App, implemented sort_session_indices with all 5 modes, integrated into rebuild_tree with AgentGroup header support

**Added sort_mode field to App, implemented sort_session_indices with all 5 modes, integrated into rebuild_tree with AgentGroup header support**

## What Happened

Added `sort_mode: SortMode` field to the App struct (defaults to TimeDesc). Implemented `cycle_sort_mode()` which advances to next mode and rebuilds the tree. Implemented `sort_session_indices()` private helper supporting all 5 sort modes: TimeDesc, TimeAsc, NameAsc, NameDesc, and AgentGroup. In `rebuild_tree()`, session indices are now sorted via `sort_session_indices()` in both search and non-search code paths. For AgentGroup mode, a static helper `append_agent_grouped()` inserts `TreeNode::AgentHeader(agent)` headers before each agent group (in fixed order: Claude, Codex, Gsd), skipping empty groups. Also added `Ord`/`PartialOrd` impls to `Agent` enum in types.rs to enable sort-by-agent. The `test_app()` helper was updated with `sort_mode: SortMode::default()`. T01's existing AgentHeader match arms in activate_selection and delete_selected were confirmed present (no-ops).

## Verification

cargo check passes (only expected dead_code warning for cycle_sort_mode which will be wired to a key in a later task). cargo test: all 49 tests pass.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo check 2>&1` | 0 | ✅ pass | 360ms |
| 2 | `cargo test 2>&1` | 0 | ✅ pass | 860ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/app/mod.rs`
- `src/types.rs`
