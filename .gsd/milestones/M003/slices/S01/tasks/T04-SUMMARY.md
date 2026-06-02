---
id: T04
parent: S01
milestone: M003
key_files:
  - src/app/ui.rs
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-02T15:57:55.865Z
blocker_discovered: false
---

# T04: Render search prompt with filter indicator at bottom of sidebar when InputMode::Search is active, and update sidebar block title to show active query

**Render search prompt with filter indicator at bottom of sidebar when InputMode::Search is active, and update sidebar block title to show active query**

## What Happened

Modified `render_sidebar()` in `src/app/ui.rs` to handle the search mode:

1. **Sidebar title**: When `input_mode == InputMode::Search`, the sidebar block title changes from " Workspaces " to ` [search: {query}] ` showing the current search query.

2. **Search prompt line**: When searching, the sidebar area is split into a tree area (top, `Min(3)`) and a 1-row search prompt (bottom, `Length(1)`). The prompt shows " search: {query}|" with a blinking cursor indicator, followed by a match count ("N matches" / "1 match" / "0 matches").

3. **Non-overlap**: The search prompt is rendered below the tree list within the block's inner area, so tree items and the prompt never overlap.

4. Clean compile with zero warnings after refactoring the match count logic.

## Verification

`cargo check` passes with no warnings

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo check` | 0 | ✅ pass | 210ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/app/ui.rs`
