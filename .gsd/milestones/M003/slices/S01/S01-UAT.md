# S01: Fuzzy search mode — UAT

**Milestone:** M003
**Written:** 2026-06-02T16:04:39.425Z

# S01: Fuzzy search mode — UAT

**Milestone:** M003
**Written:** 2026-06-02

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: All behavior is verified through 45 automated tests including 12 fuzzy-search-specific unit tests. The feature is a terminal TUI — no browser or external service involved.

## Preconditions

- Built with `cargo build` successfully
- Test data includes sessions with known titles in test fixtures

## Smoke Test

Run `cargo test --lib` — all 45 tests pass, confirming search mode infrastructure and filtering logic work.

## Test Cases

### 1. Slash enters search mode

1. App receives `KeyCode::Char('/')` while in sidebar with `InputMode::None`
2. **Expected:** `input_mode` transitions to `InputMode::Search`, `input_buffer` is cleared

### 2. Typing filters tree in real-time

1. Enter search mode (press `/`)
2. Type characters one at a time (e.g., "fix")
3. **Expected:** After each keystroke, `rebuild_tree()` is called with updated `search_query`, tree items filtered to sessions matching "fix" via fuzzy matching across title/ID/workspace

### 3. Backspace removes last character and re-filters

1. In search mode with query "fix"
2. Press Backspace
3. **Expected:** Query becomes "fi", `rebuild_tree()` re-filters, tree shows broader results

### 4. Esc exits search and restores full tree

1. In search mode with active query
2. Press Escape
3. **Expected:** `input_mode` returns to `InputMode::None`, `search_query` set to None, `input_buffer` cleared, `rebuild_tree()` restores full unfiltered tree

### 5. Empty query shows all items

1. Enter search mode (press `/`)
2. Do not type anything (empty input_buffer)
3. **Expected:** `search_query` is None or empty, all sessions and workspaces displayed

### 6. No matches shows empty tree

1. Enter search mode, type a query that matches nothing (e.g., "zzzzz")
2. **Expected:** Tree is empty, no panic, selection clamped to valid range

### 7. Selection clamped after filter

1. Have selection on item 5 in full tree
2. Enter search and type query that reduces tree to 2 items
3. **Expected:** Selection clamped to valid index within remaining items

### 8. Sidebar header shows active query

1. Enter search mode, type "fix"
2. **Expected:** Sidebar block title displays `[search: fix]`

### 9. Search prompt rendered at sidebar bottom

1. Enter search mode
2. **Expected:** Search input line with prompt "search:" and cursor rendered at bottom of sidebar area

## Edge Cases

### Fuzzy matching across all fields

1. Sessions with title "fix bug", ID "abc123", workspace "my-project"
2. Query "fix" matches title; query "abc" matches ID prefix; query "proj" matches workspace name
3. **Expected:** All three match types work via fuzzy scoring

## Failure Signals

- Panic when typing in search mode
- Tree not updating after keystroke
- Esc not restoring full tree
- Selection out of bounds after filter
- Clippy warnings or test failures

## Not Proven By This UAT

- Visual rendering correctness of the search prompt (requires live TUI inspection)
- Keystroke timing/latency during rapid typing
- Interaction with PTY session management (search mode should NOT forward keystrokes to PTY)
