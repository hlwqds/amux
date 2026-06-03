---
id: T04
parent: S01
milestone: M005
key_files:
  - src/app/ui.rs
  - src/app/handler.rs
  - src/app/mod.rs
key_decisions:
  - (none)
duration: 
verification_result: untested
completed_at: 2026-06-02T17:48:06.445Z
blocker_discovered: false
---

# T04: Extracted tab_index_from_x helper and added 22 unit tests covering tab index calculation, title truncation, and tab bar hidden-when-empty behavior

**Extracted tab_index_from_x helper and added 22 unit tests covering tab index calculation, title truncation, and tab bar hidden-when-empty behavior**

## What Happened

Extracted a standalone `tab_index_from_x(local_x: u16, tab_width: usize, num_tabs: usize) -> Option<usize>` helper function from the inline arithmetic in `handle_mouse_click`. Refactored `handle_mouse_click` to use this helper via `super::ui::tab_index_from_x`. Added 22 unit tests in `src/app/ui.rs` covering:

- **tab_index_from_x (9 tests):** First tab, second tab, boundary clicks, beyond-last-tab returns None, zero tab_width/num_tabs returns None, single tab always zero, narrow tabs.
- **truncate_title (8 tests):** Short title fits, exact fit, long title truncation, small max_len, zero max_len, empty string, unicode-aware boundary handling, at-boundary unchanged.
- **Tab bar hidden when empty (3 tests):** build_tab_bar returns empty Line when no PTYs, default Rect has zero dimensions, handle_mouse_click ignores when no PTYs.

Made `test_app` in `src/app/mod.rs` pub(crate) so ui.rs tests can call it for integration-style tests on the App. All 88 tests pass.

## Verification

Ran `cargo test --lib` — all 88 tests pass with 0 failures. Ran `cargo test --lib -- tab_bar` to confirm all 22 new/updated tab bar tests pass specifically.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| — | No verification commands discovered | — | — | — |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/app/ui.rs`
- `src/app/handler.rs`
- `src/app/mod.rs`
