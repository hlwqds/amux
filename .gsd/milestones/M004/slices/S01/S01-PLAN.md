# S01: Sort modes and agent grouping

**Goal:** Implement SortMode enum with 5 variants, sort logic in rebuild_tree, `s` keybinding, sidebar header indicator, AgentHeader tree variant with rendering, and comprehensive tests.
**Demo:** Press `s` to cycle through 5 sort modes. Sidebar header shows current mode. Agent Group mode shows agent-type sub-headers with sessions grouped under them. Sort combines with search/filter correctly. All existing tests pass + new unit tests for sort logic.

## Must-Haves

- Complete the planned slice outcomes.

## Verification

- Run the task and slice verification checks for this slice.

## Tasks

- [x] **T01: Add SortMode enum and TreeNode::AgentHeader variant** `est:20 min`
  In `src/types.rs`:
  1. Add `SortMode` enum with 5 variants: `TimeDesc`, `TimeAsc`, `NameAsc`, `NameDesc`, `AgentGroup`.
  2. Implement `SortMode::next(&self) -> SortMode` that cycles through all variants and wraps.
  3. Implement `SortMode::label(&self) -> &'static str` returning display labels like "time ↓", "time ↑", "name A→Z", "name Z→A", "agent".
  4. Add `TreeNode::AgentHeader(Agent)` variant.
  5. Update all existing match arms on TreeNode to handle AgentHeader (mod.rs, handler.rs, ui.rs) with placeholder/todo arms.
  6. Derive Copy, Clone, Debug, PartialEq, Eq on SortMode. Default trait: TimeDesc.
  - Files: `src/types.rs`
  - Verify: cargo check

- [x] **T02: Add sort_mode field to App and implement sort logic in rebuild_tree** `est:45 min`
  In `src/app/mod.rs`:
  1. Add `sort_mode: SortMode` field to App struct (default: SortMode::TimeDesc via Default).
  2. Add `cycle_sort_mode(&mut self)` method that calls `self.sort_mode = self.sort_mode.next()` then `self.rebuild_tree()`.
  3. Add private helper `sort_session_indices(&self, indices: &mut Vec<usize>)` that sorts the index vector in-place based on `self.sort_mode`:
     - TimeDesc: sort by session.last_active descending (current default behavior)
     - TimeAsc: sort by session.last_active ascending
     - NameAsc: sort by session.title case-insensitive ascending
     - NameDesc: sort by session.title case-insensitive descending
     - AgentGroup: sort by agent type, then by last_active descending within each group
  4. In `rebuild_tree()`, after computing sess_idxs (and filtering), call `sort_session_indices` before building tree nodes.
  5. For AgentGroup mode: after sorting, iterate agent types in a fixed order, insert `TreeNode::AgentHeader(agent)` before each group's sessions, skip empty groups.
  6. Update `activate_selection()` match arm: `TreeNode::AgentHeader(_)` → return Ok(Action::None) (no-op).
  7. Update `delete_selected()` match arm: `TreeNode::AgentHeader(_)` → return Ok(Action::None) (no-op).
  8. Ensure all `rebuild_tree()` call sites benefit from sort (toggle_agent_filter, refresh_sessions, handle_search_key all call rebuild_tree).
  - Files: `src/app/mod.rs`
  - Verify: cargo check && cargo test

- [ ] **T03: Add `s` keybinding to cycle sort mode** `est:10 min`
  In `src/app/handler.rs`:
  1. In sidebar key dispatch section, add `KeyCode::Char('s')` case.
  2. Call `self.cycle_sort_mode()`.
  3. Return `Ok(Action::None)` (no screen transition needed, rebuild_tree handles the tree update).
  4. Place after the agent filter keys (1/2/3) and before search key (/).
  - Files: `src/app/handler.rs`
  - Verify: cargo check && cargo test

- [ ] **T04: Render sort mode indicator and AgentHeader nodes in sidebar UI** `est:25 min`
  In `src/app/ui.rs`:
  1. Update sidebar header title to include sort mode label. Current header shows filter indicators from M003. Append sort indicator like `[sort: time ↓]`.
  2. Add `TreeNode::AgentHeader(agent)` match arm in session rendering section:
     - Render as indented line: `"  ── {agent_name} ──"` with appropriate styling (dim/gray).
     - Agent names: Claude → "Claude", Codex → "Codex", GSD → "GSD".
     - Use a distinct Style (e.g., dim + cyan or similar) to differentiate from session/workspace rows.
  3. Ensure selected AgentHeader row still shows highlight (since it's navigable with j/k).
  - Files: `src/app/ui.rs`
  - Verify: cargo check

- [ ] **T05: Add unit tests for sort logic and AgentHeader inertness** `est:40 min`
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
  - Files: `src/app/mod.rs`
  - Verify: cargo test && cargo clippy -- -D warnings && cargo fmt --check

## Files Likely Touched

- src/types.rs
- src/app/mod.rs
- src/app/handler.rs
- src/app/ui.rs
