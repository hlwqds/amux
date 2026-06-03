---
id: T02
parent: S01
milestone: M005
key_files:
  - src/app/mod.rs
  - src/app/handler.rs
key_decisions:
  - handle_mouse_click uses simple integer division for tab index calculation (tab_width = rect.width / ptys.len()) rather than tracking per-tab pixel boundaries
  - Mouse dispatch filters only Left-button Down events to avoid interfering with scroll/drag
duration: 
verification_result: passed
completed_at: 2026-06-02T17:39:36.288Z
blocker_discovered: false
---

# T02: Added tab_bar_rect field, handle_mouse_click method, and Event::Mouse dispatch for tab bar click-to-switch

**Added tab_bar_rect field, handle_mouse_click method, and Event::Mouse dispatch for tab bar click-to-switch**

## What Happened

Added three changes to implement mouse-driven tab switching:

1. **tab_bar_rect field**: Added `tab_bar_rect: Rect` field to the `App` struct (initialized to `Rect::default()` in both `App::new()` and `test_app()`), providing storage for the rendered tab bar's screen coordinates.

2. **handle_mouse_click method**: Added `pub(super) fn handle_mouse_click(&mut self, x: u16, y: u16)` in `src/app/handler.rs`. The method:
   - Returns early if no PTYs or zero-area rect
   - Bounds-checks click against `tab_bar_rect`
   - Calculates tab width as `rect.width / ptys.len()`
   - Maps local x coordinate to tab index via integer division
   - Switches to the clicked tab (different from current), resets scroll, and updates status message

3. **Event::Mouse dispatch**: Added `Event::Mouse` arm in the main event loop in `src/app/mod.rs`, matching only `MouseEventKind::Down(Left)` and delegating to `handle_mouse_click`.

Added 6 unit tests covering: no-pty early return, zero-rect early return, outside-bounds click ignoring, and tab index calculation for single tab, multiple tabs, and offset-rect scenarios. All 67 tests pass (6 new + 61 existing). Clippy passes with zero warnings.

## Verification

Ran `cargo clippy -- -D warnings` — compiles with zero warnings. Ran `cargo test` — all 67 tests pass (6 new handle_mouse_click tests + 61 existing).

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo clippy -- -D warnings 2>&1 | tail -5` | 0 | ✅ pass | 385ms |
| 2 | `cargo test 2>&1 | grep 'test result'` | 0 | ✅ pass | 683ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/app/mod.rs`
- `src/app/handler.rs`
