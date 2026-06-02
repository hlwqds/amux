---
estimated_steps: 7
estimated_files: 1
skills_used: []
---

# T01: Add agent_filter field and integrate into rebuild_tree

1. Add `agent_filter: Option<Agent>` field to App struct in `src/app/mod.rs`
2. Initialize to `None` in `App::new()`
3. Modify `rebuild_tree()` to also check `self.agent_filter`:
   - When agent_filter is Some(agent), exclude sessions whose agent type does not match
   - Combined with text search: both predicates must pass (intersection)
   - Only include workspaces that have at least one matching session after both filters
4. Run `cargo check`

## Inputs

- `src/app/mod.rs`

## Expected Output

- `App struct with agent_filter field`
- `rebuild_tree with agent filter logic`

## Verification

cargo check && cargo test
