# M003: Search and Filter

**Vision:** Add live fuzzy search and agent-type filtering to the sidebar tree. Press `/` to enter search mode with a live input prompt that filters workspaces and sessions via fuzzy matching. Press `1`/`2`/`3` to toggle agent-type filters (Claude, Codex, GSD) that combine with text search. Press `Esc` to clear all filters and restore the full tree.

## Success Criteria

- `/` enters search mode with visible input prompt
- Typing in search mode filters tree in real-time via fuzzy matching
- Fuzzy matching covers session title, session ID prefix, and workspace name
- `1`/`2`/`3` toggles agent filter; combining with text search works
- Esc exits search mode, clears query and filter, restores full tree
- Sidebar header shows active filter state (e.g., [search: fix] [GSD])
- All existing 33+ tests pass, cargo clippy -- -D warnings exits 0, cargo fmt check clean

## Slices

- [ ] **S01: Fuzzy search mode** `risk:medium` `depends:[]`
  > After this: Press `/` in sidebar → type "fix" → tree filters to sessions with "fix" in title/workspace/ID → press Esc → full tree restored

- [ ] **S02: Agent-type toggle filter** `risk:low` `depends:[S01]`
  > After this: Press `3` in sidebar → only GSD sessions shown → type "fix" → only GSD sessions matching "fix" → press `3` again → filter cleared, text search still active

## Boundary Map

Not provided.
