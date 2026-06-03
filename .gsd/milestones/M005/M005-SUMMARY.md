---
id: M005
title: "PTY Tab Bar"
status: complete
completed_at: 2026-06-02T18:02:29.758Z
key_decisions:
  - Active tab uses Rgb(24,36,72) bg matching sidebar highlight style
  - Equal-width tab division for rendering and mouse handling consistency
  - truncate_title uses char_indices() for correct multi-byte unicode handling
  - Mouse events filtered to only Left-button Down to avoid interfering with scroll/drag
key_files:
  - src/util.rs
  - src/app/mod.rs
  - src/app/handler.rs
  - src/app/ui.rs
lessons_learned:
  - Equal-width tab division simplifies coordinate mapping but truncates long titles aggressively regardless of neighbor title lengths
  - Mouse capture enabled for entire terminal session may interfere with terminal-native selection/copy — consider making it toggleable
  - char_indices() is essential for truncate_title to handle multi-byte unicode correctly — .chars().take(n) would produce wrong byte offsets
---

# M005: PTY Tab Bar

**Complete PTY tab bar with mouse-driven tab switching, agent-colored icons, state indicators, title truncation, and 88 passing tests**

## What Happened

M005 added a horizontal tab bar to the chat area showing all active PTY sessions with agent-colored icons, truncated titles, and running/done state indicators. The tab bar appears when the first PTY is spawned and disappears when all PTYs are closed. Mouse press-to-switch enables direct tab navigation, while Ctrl+J/K keyboard cycling and Ctrl+Q close continue to work. Active tabs use Rgb(24,36,72) background matching the sidebar highlight style. Equal-width tab division ensures consistent coordinate mapping between rendering and mouse handling. The implementation is covered by 88 passing unit tests with clippy clean. Known limitations: equal-width tab division (no title-length adaptation), mouse capture potentially interfering with terminal-native selection, and no scroll/overflow for 20+ sessions.

## Success Criteria Results

- [x] Tab bar appears when PTY sessions are active — ui.rs gates on !self.ptys.is_empty(), test tab_bar_hidden_when_no_ptys
- [x] Each tab shows agent icon, truncated title, state indicator — build_tab_bar() with 22 tab_bar tests
- [x] Active tab visually distinct — Rgb(24,36,72) bg + BOLD modifier
- [x] Mouse press-to-switch works — handle_mouse_click() via tab_index_from_x, 6 mouse + 9 index tests
- [x] Ctrl+J/K cycling preserved — handler.rs:30-36 unchanged
- [x] Ctrl+Q closes tab — handler.rs:21,85, re-renders from live state
- [x] Tab bar disappears when no PTYs — !self.ptys.is_empty() gate
- [x] Title truncation with ... for overflow — truncate_title() with char_indices(), 8 tests
- [x] No regressions — 88/88 tests pass, clippy clean

## Definition of Done Results

- All acceptance criteria verified with automated tests (88/88)
- Clippy clean with -D warnings
- Code changes limited to src/util.rs, src/app/mod.rs, src/app/handler.rs, src/app/ui.rs
- Summary and assessment artifacts persisted
- Known limitations documented (equal-width tabs, mouse capture, no overflow)

## Requirement Outcomes

Not provided.

## Deviations

None.

## Follow-ups

Consider adding tab-width adaptation based on title length, mouse capture toggle to restore terminal-native selection, and scroll/overflow for 20+ tabs.
