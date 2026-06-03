---
id: S01
parent: M004
milestone: M004
provides:
  - 5 sort modes cycled via s key in sidebar
  - Agent Group mode with agent-type sub-headers
  - Sort mode indicator in sidebar header
  - 12 unit tests covering sort logic
requires:
  []
affects:
  []
key_files:
  - src/types.rs
  - src/app/mod.rs
  - src/app/handler.rs
  - src/app/ui.rs
key_decisions:
  - Agent Ord/PartialOrd uses canonical pattern: Ord::cmp does u8 comparison, PartialOrd delegates to cmp
  - Sort is applied after filter in rebuild_tree, ensuring sort mode changes don't alter filtered set
patterns_established:
  - SortMode enum with cycle pattern — reusable for any future cycling enum
  - TreeNode::AgentHeader separator pattern — inert navigable node for visual grouping
observability_surfaces:
  - none
drill_down_paths:
  - .gsd/milestones/M004/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M004/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M004/slices/S01/tasks/T03-SUMMARY.md
  - .gsd/milestones/M004/slices/S01/tasks/T04-SUMMARY.md
  - .gsd/milestones/M004/slices/S01/tasks/T05-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-02T17:10:33.203Z
blocker_discovered: false
---

# S01: Sort modes and agent grouping

**Implemented 5 sort modes (Time Desc/Asc, Name A→Z/Z→A, Agent Group) cycled via `s` key, with sidebar header indicator and AgentHeader group separator nodes**

## What Happened

The slice added session sorting and agent grouping to the sidebar across 5 tasks:

**T01** added the `SortMode` enum with 5 variants (TimeDesc, TimeAsc, NameAsc, NameDesc, AgentGroup), cycle/label methods, and the `TreeNode::AgentHeader` variant. All existing match arms were updated across mod.rs, handler.rs, ui.rs, and session.rs.

**T02** added `sort_mode: SortMode` to the App struct, implemented `sort_session_indices()` with mode-specific sorting logic, and integrated it into `rebuild_tree()` after filtering. AgentGroup mode inserts `TreeNode::AgentHeader` separators before each agent type's sessions. Inert behavior was wired for activate/delete on AgentHeader nodes.

**T03** added the `s` keybinding in sidebar mode, calling `cycle_sort_mode()` which advances the mode and rebuilds the tree.

**T04** updated the sidebar header to show the current sort mode label and rendered AgentHeader nodes as styled dim separator lines ("── AgentName ──").

**T05** added 12 unit tests covering all 5 sort modes, filter integration, selection clamping, and AgentHeader inertness. Also fixed pre-existing clippy warnings (Agent Ord/PartialOrd canonical pattern, ptr_arg lint) and ran cargo fmt.

All 61 tests pass, clippy is clean with -D warnings, and cargo fmt --check passes.

## Verification

Full verification suite passed:
- `cargo test`: 61 tests pass (49 existing + 12 new sort-specific tests), 0 failures
- `cargo clippy -- -D warnings`: zero warnings
- `cargo fmt --check`: clean
- All 5 sort modes verified by dedicated unit tests
- AgentHeader inertness verified for both activate and delete operations
- Filter + sort integration verified
- Selection clamping after sort change verified

## Requirements Advanced

None.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None — all 5 tasks completed as planned with no deviations.

## Known Limitations

Agent Group mode shows a fixed order of agent types. If a new agent type is added, it will appear at whatever position the Agent enum ordinals place it, not alphabetically or in a configurable order.

## Follow-ups

None — single-slice milestone, no downstream slices.

## Files Created/Modified

None.
