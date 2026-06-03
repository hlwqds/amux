# M005: PTY Tab Bar

**Vision:** Add a horizontal tab bar at the top of the chat area showing all active PTY sessions with agent icon, truncated title, and running/done state. Enable mouse-driven tab switching alongside existing Ctrl+J/K keyboard navigation. The tab bar appears when the first PTY is spawned and disappears when all PTYs are closed.

## Slices

- [x] **S01: PTY tab bar with mouse switching** `risk:low` `depends:[]`
  > After this: Spawn 2+ agent sessions, see tab bar with colored agent icons and titles, click a tab to switch, use Ctrl+J/K to cycle, Ctrl+Q to close a tab — tab bar updates immediately in all cases. Tab bar hidden when no PTYs active.

## Boundary Map

Not provided.
