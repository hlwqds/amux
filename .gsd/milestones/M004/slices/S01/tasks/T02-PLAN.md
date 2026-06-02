---
estimated_steps: 14
estimated_files: 1
skills_used: []
---

# T02: Add sort_mode field to App and implement sort logic in rebuild_tree

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

## Inputs

- `src/app/mod.rs`
- `src/types.rs`

## Expected Output

- `src/app/mod.rs with sort_mode field and sorted rebuild_tree`

## Verification

cargo check && cargo test
