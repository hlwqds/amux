---
id: T03
parent: S01
milestone: M005
key_files:
  - src/app/ui.rs
key_decisions:
  - Active tab uses Rgb(24,36,72) bg matching sidebar highlight style; inactive tabs use DarkGray fg for all elements including state indicators
  - truncate_title operates on byte indices from char_indices() to correctly handle multi-byte unicode characters, truncating at char boundaries
  - Tab bar width calculation uses simple (width - separators) / n_tabs equal division, consistent with T02's handle_mouse_click integer division approach
duration: 
verification_result: passed
completed_at: 2026-06-02T17:45:36.276Z
blocker_discovered: false
---

# T03: Rendered PTY tab bar in chat area with layout split, agent-colored icons, state indicators, click-to-switch coordinate mapping, and title truncation

**Rendered PTY tab bar in chat area with layout split, agent-colored icons, state indicators, click-to-switch coordinate mapping, and title truncation**

## What Happened

Modified `render_chat()` in `src/app/ui.rs` to split the inner area into a 1-row tab bar and the remaining PTY content area when PTYs are active. The tab bar shows each PTY as a segment with agent icon (colored by agent), truncated title, and state indicator (● running/✔ done). Active tab gets highlighted bg (Rgb(24,36,72)) while inactive tabs use DarkGray fg. Tabs are separated by │. The `tab_bar_rect` is stored for T02's mouse click-to-switch coordinate mapping.

Added `build_tab_bar(&self, width)` method that constructs the tab Line with Spans for each tab, calculating equal-width segments with separators. Added `truncate_title(title, max_len)` standalone function that truncates at char boundaries and appends "...". When no PTYs are active, the existing placeholder path is used unchanged.

Added 6 unit tests covering: fits-within-limit, exact-fit, long-title truncation, small max_len (≤3 returns original), zero max_len, and unicode-aware truncation. All 73 tests pass (67 existing + 6 new), clippy clean with zero warnings.

## Verification

Ran `cargo clippy -- -D warnings` — compiles with zero warnings. Ran `cargo test` — all 73 tests pass (67 existing + 6 new truncate_title tests).

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo clippy -- -D warnings 2>&1 | tail -5` | 0 | ✅ pass | 334ms |
| 2 | `cargo test 2>&1 | grep 'test result'` | 0 | ✅ pass | 175ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/app/ui.rs`
