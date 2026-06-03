---
estimated_steps: 13
estimated_files: 2
skills_used: []
---

# T04: Unit tests for tab index calculation and title truncation

Add unit tests in `src/app/ui.rs` (or `src/app/mod.rs` test module):

1. `test_tab_index_from_click` — given tab_bar_rect, number of ptys, and click x coordinate, verify correct tab index is calculated:
   - Click on first tab → index 0
   - Click on second tab → index 1
   - Click beyond last tab → ignored
   - Click on current tab → no switch

2. `test_truncate_title` — verify title truncation:
   - Short title fits → unchanged
   - Long title → truncated with `...` suffix
   - Empty title → empty string
   - Title exactly at max length → unchanged

3. `test_tab_bar_hidden_when_empty` — verify that when `ptys` is empty, no tab bar rect is stored / rendering skips tab bar.

Add a public helper function `tab_index_from_x(local_x: u16, tab_width: usize, num_tabs: usize) -> Option<usize>` that can be tested independently.

## Inputs

- `src/app/ui.rs`
- `src/app/mod.rs`

## Expected Output

- `src/app/ui.rs`

## Verification

cargo test 2>&1 | tail -20
