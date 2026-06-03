# M004: Session Sorting and Grouping

**Vision:** Add 5 sort modes (Time ↓, Time ↑, Name A→Z, Name Z→A, Agent Group) to the sidebar, cycled via `s` key. Sort applies after M003's search/filter. Agent Group mode shows agent-type sub-headers. Single slice: types enum → App field + rebuild_tree logic → `s` keybinding → UI rendering → tests.

## Success Criteria

- `s` key cycles through 5 sort modes: Time Desc → Time Asc → Name A→Z → Name Z→A → Agent Group
- Sidebar header shows current sort mode indicator
- Time Desc is default, matching current behavior
- Name sorts are case-insensitive alphabetical
- Agent Group mode shows agent-type sub-headers with sessions grouped under them
- Sort applies after search/filter — changing sort while searching doesn't change filtered set
- AgentHeader nodes navigable with j/k but inert for Enter and D
- No regressions: existing tests pass, clippy clean, fmt clean

## Slices

- [x] **S01: Sort modes and agent grouping** `risk:medium` `depends:[]`
  > After this: Press `s` to cycle through 5 sort modes. Sidebar header shows current mode. Agent Group mode shows agent-type sub-headers with sessions grouped under them. Sort combines with search/filter correctly. All existing tests pass + new unit tests for sort logic.

## Boundary Map

Not provided.
