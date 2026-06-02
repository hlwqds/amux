---
estimated_steps: 7
estimated_files: 1
skills_used: []
---

# T01: Add SortMode enum and TreeNode::AgentHeader variant

In `src/types.rs`:
1. Add `SortMode` enum with 5 variants: `TimeDesc`, `TimeAsc`, `NameAsc`, `NameDesc`, `AgentGroup`.
2. Implement `SortMode::next(&self) -> SortMode` that cycles through all variants and wraps.
3. Implement `SortMode::label(&self) -> &'static str` returning display labels like "time ↓", "time ↑", "name A→Z", "name Z→A", "agent".
4. Add `TreeNode::AgentHeader(Agent)` variant.
5. Update all existing match arms on TreeNode to handle AgentHeader (mod.rs, handler.rs, ui.rs) with placeholder/todo arms.
6. Derive Copy, Clone, Debug, PartialEq, Eq on SortMode. Default trait: TimeDesc.

## Inputs

- `src/types.rs`

## Expected Output

- `src/types.rs with SortMode enum and TreeNode::AgentHeader variant`

## Verification

cargo check
