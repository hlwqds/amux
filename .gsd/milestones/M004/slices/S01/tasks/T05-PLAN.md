---
estimated_steps: 14
estimated_files: 1
skills_used: []
---

# T05: Add unit tests for sort logic and AgentHeader inertness

In `src/app/mod.rs` test module:
1. `sort_mode_cycles_through_all_variants` — verify next() visits all 5 variants and wraps.
2. `sort_mode_default_is_time_desc` — verify Default is TimeDesc.
3. `sort_time_desc_newest_first` — sessions sorted by last_active descending.
4. `sort_time_asc_oldest_first` — sessions sorted by last_active ascending.
5. `sort_name_asc_alphabetical` — case-insensitive alphabetical by title.
6. `sort_name_desc_reverse_alphabetical` — case-insensitive reverse alphabetical.
7. `sort_agent_group_groups_by_agent` — sessions grouped by agent type with headers.
8. `sort_agent_group_omits_empty_groups` — no header for agent type with zero sessions.
9. `sort_with_active_filter` — sort reorders filtered results; filter is applied first.
10. `agent_header_is_inert_for_activate` — activate_selection on AgentHeader returns None.
11. `agent_header_is_inert_for_delete` — delete_selected on AgentHeader returns None.
12. `sort_preserves_selection_clamping` — after changing sort mode, selection is clamped to valid range.

All tests should use the existing test helper pattern (create app, add sessions, set sort_mode, call rebuild_tree, assert tree order).

## Inputs

- `src/app/mod.rs`
- `src/types.rs`

## Expected Output

- `All existing + new tests pass, clippy clean, fmt clean`

## Verification

cargo test && cargo clippy -- -D warnings && cargo fmt --check
