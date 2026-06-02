---
id: T01
parent: S01
milestone: M004
key_files:
  - src/types.rs
  - src/app/mod.rs
  - src/app/ui.rs
  - src/app/session.rs
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-02T16:58:19.859Z
blocker_discovered: false
---

# T01: Added SortMode enum (5 variants with cycle/label) and TreeNode::AgentHeader variant with all match-arm updates

**Added SortMode enum (5 variants with cycle/label) and TreeNode::AgentHeader variant with all match-arm updates**

## What Happened

Added `SortMode` enum to `src/types.rs` with 5 variants (`TimeDesc`, `TimeAsc`, `NameAsc`, `NameDesc`, `AgentGroup`), deriving `Copy`, `Clone`, `Debug`, `Default` (→ `TimeDesc`), `PartialEq`, `Eq`. Implemented `SortMode::next()` cycling through all variants, and `SortMode::label()` returning display strings ("time ↓", "time ↑", "name A→Z", "name Z→A", "agent").

Added `TreeNode::AgentHeader(Agent)` variant to the existing `TreeNode` enum. Updated all exhaustive match arms across 4 files:
- `src/app/mod.rs`: `activate_selection()` and `delete_selected()` — placeholder no-op arms
- `src/app/handler.rs`: no direct TreeNode matches (delegates to methods already updated)
- `src/app/ui.rs`: sidebar rendering (agent-colored group header with ▸ icon) and detail placeholder view
- `src/app/session.rs`: `spawn_with_agent()` — placeholder no-op arm

Fixed a lifetime issue where the `AgentHeader` detail view borrowed a local `Agent` into `Line<'static>` by converting the label to an owned `String`. All 49 existing tests pass.

## Verification

`cargo check` passes cleanly. `cargo test` runs all 49 tests with 0 failures.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo check` | 0 | ✅ pass | 500ms |
| 2 | `cargo test` | 0 | ✅ pass (49 tests) | 1500ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/types.rs`
- `src/app/mod.rs`
- `src/app/ui.rs`
- `src/app/session.rs`
