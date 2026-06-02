# S02: Agent-type toggle filter

**Goal:** Add agent-type toggle filter (keys 1/2/3) that composes with text search, plus combined filter indicator rendering in sidebar header
**Demo:** Press `3` in sidebar → only GSD sessions shown → type "fix" → only GSD sessions matching "fix" → press `3` again → filter cleared, text search still active

## Must-Haves

- Key 1 toggles Claude-only filter, 2 Codex, 3 GSD; pressing same key again clears it
- Agent filter combines with text search (intersection)
- Workspaces with zero matching sessions are hidden
- Sidebar header shows both [search: query] and [Agent] when both active
- Esc clears both text search and agent filter
- Agent filter works independently of text search
- All existing tests pass, clippy clean

## Proof Level

- This slice proves: contract

## Integration Closure

Agent filter uses the same rebuild_tree filter path as text search. No new integration points beyond existing sidebar key routing.

## Verification

- Sidebar header shows [Claude/Codex/GSD] when agent filter is active, combined with [search: query] when both active

## Tasks

- [ ] **T01: Add agent_filter field and integrate into rebuild_tree** `est:10 min`
  1. Add `agent_filter: Option<Agent>` field to App struct in `src/app/mod.rs`
  2. Initialize to `None` in `App::new()`
  3. Modify `rebuild_tree()` to also check `self.agent_filter`:
     - When agent_filter is Some(agent), exclude sessions whose agent type does not match
     - Combined with text search: both predicates must pass (intersection)
     - Only include workspaces that have at least one matching session after both filters
  4. Run `cargo check`
  - Files: `src/app/mod.rs`
  - Verify: cargo check && cargo test

- [ ] **T02: Add 1/2/3 keybindings for agent filter toggle** `est:10 min`
  1. In `src/app/handler.rs`, add keybindings in the sidebar key match section:
     - `KeyCode::Char('1')` toggles `agent_filter` to/from `Some(Agent::Claude)`
     - `KeyCode::Char('2')` toggles to/from `Some(Agent::Codex)`
     - `KeyCode::Char('3')` toggles to/from `Some(Agent::Gsd)`
     - Each toggle calls `rebuild_tree()`
  2. Update Esc handler in search mode to also clear `agent_filter`
  3. Also handle agent filter toggle when NOT in search mode (sidebar idle)
  4. Run `cargo check`
  - Files: `src/app/handler.rs`
  - Verify: cargo check

- [ ] **T03: Render combined filter indicators in sidebar header** `est:10 min`
  1. In `src/app/ui.rs`, update `render_sidebar()` block title:
     - Show `[search: query]` when text search is active
     - Show `[Claude/Codex/GSD]` when agent filter is active
     - Show both when both are active
     - Show plain `Workspaces` when no filters active
  2. Run `cargo check`
  - Files: `src/app/ui.rs`
  - Verify: cargo check

- [ ] **T04: Verify agent filter and combined search+filter** `est:15 min`
  1. Run `cargo test` — all existing tests must pass
  2. Run `cargo clippy -- -D warnings` — zero warnings
  3. Run `cargo fmt --check` — clean
  4. Build and manually verify:
     - Press 3, only GSD sessions shown
     - Type query with 3 active, intersection works
     - Press 3 again, filter cleared
     - Esc clears both
  5. Add unit tests:
     - Test: agent filter alone shows only matching agent sessions
     - Test: agent filter + text search intersection
     - Test: workspace with zero matching sessions is hidden
     - Test: toggle same key clears filter
  - Files: `src/app/mod.rs`
  - Verify: cargo test && cargo clippy -- -D warnings && cargo fmt --check

## Files Likely Touched

- src/app/mod.rs
- src/app/handler.rs
- src/app/ui.rs
