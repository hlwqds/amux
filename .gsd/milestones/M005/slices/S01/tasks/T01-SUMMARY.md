---
id: T01
parent: S01
milestone: M005
key_files:
  - src/util.rs
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-02T17:35:42.168Z
blocker_discovered: false
---

# T01: Added EnableMouseCapture/DisableMouseCapture to terminal init/restore lifecycle

**Added EnableMouseCapture/DisableMouseCapture to terminal init/restore lifecycle**

## What Happened

Added `EnableMouseCapture` to `init_terminal()` and `DisableMouseCapture` to `restore_terminal()` via the `execute!` macro alongside the existing `EnterAlternateScreen`/`LeaveAlternateScreen` calls. Both imports were added to the existing `crossterm::event` import block. Clippy passes with zero warnings and all existing tests pass.

## Verification

Ran `cargo clippy -- -D warnings` — compiles with zero warnings. Ran `cargo test` — all tests pass.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo clippy -- -D warnings 2>&1 | tail -5` | 0 | ✅ pass | 357ms |
| 2 | `cargo test 2>&1 | tail -10` | 0 | ✅ pass | 573ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/util.rs`
