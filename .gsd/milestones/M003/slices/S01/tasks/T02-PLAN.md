---
estimated_steps: 9
estimated_files: 1
skills_used: []
---

# T02: Add search fields to App and modify rebuild_tree with filter logic

1. Add `search_query: Option<String>` field to App struct in `src/app/mod.rs`
2. Initialize to `None` in `App::new()`
3. Modify `rebuild_tree()` to read `self.search_query` and filter sessions/workspaces:
   - When query is Some(non-empty), score each session's title + short ID + workspace name against query using `code_fuzzy_match::fuzzy_match`
   - Only include sessions with a positive score
   - Only include workspaces that have at least one matching session or match the query themselves
   - When query is None or empty, show all items (no filtering)
4. After rebuilding tree, clamp selection to valid range (call `move_sel(0)` if tree is non-empty)
5. Run `cargo check`

## Inputs

- `src/app/mod.rs`
- `src/types.rs`

## Expected Output

- `Modified App struct with search_query field`
- `Modified rebuild_tree with fuzzy filter logic`

## Verification

cargo check && cargo test
