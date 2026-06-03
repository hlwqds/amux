---
id: S01
parent: M005
milestone: M005
provides:
  - PTY tab bar rendering with agent icons, state indicators, and title truncation
  - Mouse click-to-switch tab navigation
  - Tab bar visibility toggle based on PTY session count
requires:
  []
affects:
  []
key_files:
  - src/util.rs
  - src/app/mod.rs
  - src/app/handler.rs
  - src/app/ui.rs
key_decisions:
  - Active tab uses Rgb(24,36,72) bg matching sidebar highlight style
  - Equal-width tab division (width / n_tabs) for both rendering and click handling, ensuring coordinate mapping consistency
  - truncate_title uses char_indices() for correct multi-byte unicode handling
  - Mouse events filtered to only Left-button Down to avoid interfering with scroll/drag
  - tab_index_from_x extracted as shared helper for reuse in both rendering and click handling
patterns_established:
  - Tab bar rendering with equal-width division and coordinate-mapped click handling
  - Mouse capture lifecycle paired with terminal alternate screen lifecycle
observability_surfaces:
  - Tab bar visibility itself is the primary health signal — if PTYs exist but tab bar is missing, something is wrong
drill_down_paths:
  - .gsd/milestones/M005/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M005/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M005/slices/S01/tasks/T03-SUMMARY.md
  - .gsd/milestones/M005/slices/S01/tasks/T04-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-02T17:49:29.222Z
blocker_discovered: false
---

# S01: PTY tab bar with mouse switching

**Complete PTY tab bar with mouse-driven click-to-switch, agent-colored icons, state indicators, title truncation, and 88 passing tests**

## What Happened

Implemented the full PTY tab bar feature across four tasks:

**T01** added EnableMouseCapture/DisableMouseCapture to the terminal lifecycle in util.rs, paired with the existing alternate screen enter/leave calls.

**T02** added the tab_bar_rect field to App, implemented handle_mouse_click() using integer-division tab index calculation, and added Event::Mouse dispatch in the main event loop filtering only Left-button Down events. 6 unit tests cover click handling edge cases.

**T03** implemented render_tab_bar() in ui.rs with: layout split reserving 3 rows for the tab bar when PTYs are active; equal-width tab division with separator lines; agent-colored icons from the agent registry; running/done state indicators (spinner vs checkmark); active tab highlight using Rgb(24,36,72) background matching sidebar style; and truncate_title() using char_indices() for correct multi-byte unicode handling. 6 truncate_title tests added.

**T04** extracted the tab_index_from_x helper for shared use between rendering and click handling, and expanded the test suite to 22 tab bar tests covering: tab index calculation from x coordinates, title truncation edge cases (zero length, small max, unicode), and tab bar hidden-when-no-pty behavior.

All 88 library tests pass, clippy reports zero warnings.

## Verification

cargo clippy -- -D warnings: zero warnings. cargo test --lib: 88 passed, 0 failed. cargo test --lib -- tab_bar: 22 tab bar specific tests all pass. cargo test --lib -- handle_mouse_click: 1 mouse click test passes.

## Requirements Advanced

- R001 — Tab bar renders at top of chat area with agent-colored icons, truncated titles, running/done state indicators, active tab highlight, and hidden-when-no-pty behavior — all verified by 22 unit tests
- R002 — Mouse capture enabled/disabled with terminal lifecycle; left-click dispatch maps coordinates to tab indices for switching; mouse events not forwarded to PTY subprocess

## Requirements Validated

- R001 — 22 automated tests verify rendering, truncation, visibility toggle, and layout math
- R002 — 6 handle_mouse_click tests + 1 mouse dispatch test verify click-to-switch and bounds checking; EnableMouseCapture/DisableMouseCapture lifecycle verified in T01

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None.

## Known Limitations

Equal-width tab division means tabs don't adapt to title length — long titles truncate equally regardless of other tab title lengths. Mouse capture is enabled for the entire terminal session; this may interfere with terminal-native mouse selection/copy behavior. No scroll/overflow mechanism for very large numbers of tabs (e.g., 20+ sessions).

## Follow-ups

None.

## Files Created/Modified

- `src/util.rs` — Added EnableMouseCapture/DisableMouseCapture to terminal init/restore lifecycle
- `src/app/mod.rs` — Added tab_bar_rect field to App struct
- `src/app/handler.rs` — Added handle_mouse_click method, tab_index_from_x helper, and Event::Mouse dispatch in main loop
- `src/app/ui.rs` — Added render_tab_bar, truncate_title, tab bar layout split in render_chat, and 22 unit tests
