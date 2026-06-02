---
id: T02
parent: S01
milestone: M003
key_files:
  - src/app/mod.rs
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-02T15:53:37.566Z
blocker_discovered: false
---

# T02: Added search_query field to App and implemented fuzzy-filter logic in rebuild_tree using code_fuzzy_match

**Added search_query field to App and implemented fuzzy-filter logic in rebuild_tree using code_fuzzy_match**

## What Happened

Added `search_query: Option<String>` field to the `App` struct in `src/app/mod.rs`, initialized to `None` in `App::new()`.

Modified `rebuild_tree()` to support fuzzy filtering:
- When `search_query` is `Some(non-empty)`, each session's title and short ID (first 8 chars) are scored against the query using `code_fuzzy_match::fuzzy_match`. Active PTYs are also filtered by their title.
- Only sessions with a positive fuzzy score are included. Workspaces are only shown if they match the query themselves or have at least one matching child (session or active PTY).
- When query is `None` or empty, the original behavior is preserved (show all items, respect expanded state).
- After rebuilding the tree, selection is clamped to valid range via `move_sel(0)`.

Added a module-level helper `session_fuzzy_score(title, short_id, query) -> bool` that wraps `code_fuzzy_match::fuzzy_match` and checks both title and short_id haystacks.

`cargo check` passes cleanly (0 warnings). All 33 existing tests pass.

## Verification

`cargo check` passes with zero warnings. `cargo test` passes all 33 tests across lib, main, and doc-test suites.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo check 2>&1` | 0 | ✅ pass | 200ms |
| 2 | `cargo test 2>&1` | 0 | ✅ pass | 1150ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/app/mod.rs`
