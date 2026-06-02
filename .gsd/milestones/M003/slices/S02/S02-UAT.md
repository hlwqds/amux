# S02: Agent-type toggle filter — UAT

**Milestone:** M003
**Written:** 2026-06-02T16:20:58.758Z

# S02: Agent-type toggle filter — UAT

**Milestone:** M003
**Written:** 2026-06-02

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: All behavior is verified by unit tests (49 passing) and build checks (clippy, fmt). The feature is a filter on in-memory data with no external dependencies or runtime state that requires live verification.

## Preconditions

- Project compiles (`cargo build`)
- Test data includes sessions with Claude, Codex, and GSD agent types (provided by existing test fixtures)

## Smoke Test

Run `cargo test --lib` — all 49 tests pass, including 4 agent-filter-specific tests.

## Test Cases

### 1. Agent filter alone shows only matching sessions

1. Set `app.agent_filter = Some(Agent::Claude)`
2. Call `rebuild_tree()`
3. **Expected:** Only Claude sessions appear; non-Claude sessions excluded; workspaces with no Claude sessions hidden

### 2. Agent filter + text search intersection

1. Set `app.search_query = Some("fix".to_string())` and `app.agent_filter = Some(Agent::Gsd)`
2. Call `rebuild_tree()`
3. **Expected:** Only GSD sessions with "fix" in title/workspace/ID appear

### 3. Non-matching workspaces hidden

1. Set `app.agent_filter = Some(Agent::Codex)`
2. Call `rebuild_tree()` with data where no workspace has Codex sessions
3. **Expected:** All workspaces hidden, tree is empty

### 4. Toggle same key clears filter

1. Call `toggle_agent_filter(Agent::Claude)` — sets filter to Some(Claude)
2. Call `toggle_agent_filter(Agent::Claude)` again
3. **Expected:** `agent_filter` is `None`, filter cleared

### 5. Esc clears both search and agent filter

1. Activate search mode with query and agent filter
2. Press Esc
3. **Expected:** Both `search_query` and `agent_filter` cleared, full tree restored

### 6. Sidebar header shows combined indicators

1. Activate agent filter only → header shows `[Claude]` (or Codex/GSD)
2. Activate text search only → header shows `[search: query]`
3. Activate both → header shows `[search: query] [Claude]`
4. Neither active → header shows `Workspaces`

## Edge Cases

### Agent filter with empty workspace list

1. Start with no workspaces
2. Toggle agent filter
3. **Expected:** No crash, empty tree renders correctly

### Rapid toggle

1. Toggle agent filter on and off rapidly
2. **Expected:** Final state matches last toggle action, no stale state

## Failure Signals

- Tests failing (especially the 4 agent-filter-specific tests)
- Clippy warnings in handler.rs, mod.rs, or ui.rs
- Sidebar header not updating when filters change
- Workspaces showing sessions of wrong agent type

## Not Proven By This UAT

- Live keyboard input handling (requires TUI runtime)
- Visual rendering of filter indicators (requires terminal)
- Performance with very large session counts

## Notes for Tester

This is a terminal TUI application. The unit tests cover all filtering logic exhaustively. Manual verification of the keybindings and header rendering requires running `cargo run` and pressing 1/2/3 in the sidebar.
