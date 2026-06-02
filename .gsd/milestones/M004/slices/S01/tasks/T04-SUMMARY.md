---
id: T04
parent: S01
milestone: M004
key_files:
  - src/app/ui.rs
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-02T17:03:34.452Z
blocker_discovered: false
---

# T04: Added sort mode indicator to sidebar title and styled AgentHeader nodes with dim separator rendering

**Added sort mode indicator to sidebar title and styled AgentHeader nodes with dim separator rendering**

## What Happened

Updated two areas in `src/app/ui.rs`:

1. **Sort mode indicator in sidebar title**: The title now always includes `[sort: {label}]` (e.g. `[sort: time ↓]`, `[sort: agent]`) alongside existing search/filter indicators. This applies to all four title variants (search+filter, search only, filter only, plain).

2. **AgentHeader rendering**: Changed from bold `▸ AgentName` to dim `── AgentName ──` style using the `──` box-drawing separator characters with `Modifier::DIM` and the agent's color. This visually distinguishes agent group headers from session/workspace rows while keeping them navigable with j/k.

Both the sidebar list rendering and the placeholder pane already had AgentHeader match arms from T01; only the sidebar list rendering style was updated per the task plan. `cargo check` passes cleanly.

## Verification

cargo check passes with no errors or warnings.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo check 2>&1` | 0 | ✅ pass | 3800ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/app/ui.rs`
