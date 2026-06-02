---
estimated_steps: 7
estimated_files: 1
skills_used: []
---

# T04: Render sort mode indicator and AgentHeader nodes in sidebar UI

In `src/app/ui.rs`:
1. Update sidebar header title to include sort mode label. Current header shows filter indicators from M003. Append sort indicator like `[sort: time ↓]`.
2. Add `TreeNode::AgentHeader(agent)` match arm in session rendering section:
   - Render as indented line: `"  ── {agent_name} ──"` with appropriate styling (dim/gray).
   - Agent names: Claude → "Claude", Codex → "Codex", GSD → "GSD".
   - Use a distinct Style (e.g., dim + cyan or similar) to differentiate from session/workspace rows.
3. Ensure selected AgentHeader row still shows highlight (since it's navigable with j/k).

## Inputs

- `src/app/ui.rs`
- `src/types.rs`

## Expected Output

- `src/app/ui.rs with sort indicator and AgentHeader rendering`

## Verification

cargo check
