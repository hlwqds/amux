---
depends_on: [M003]
---

# M005: PTY Tab Bar

**Gathered:** 2026-06-02
**Status:** Ready for planning

## Project Description

amux is a keyboard-first terminal UI for managing AI coding agent workspaces and sessions (Claude Code, Codex, GSD). Written in Rust (edition 2024) using ratatui + crossterm + portable-pty. The codebase is modular after M002: `app/mod.rs` with sub-modules for UI, handler, session, and browse concerns. M003 adds search/filter. M004 adds sorting/grouping.

## Why This Milestone

When multiple agent sessions are running simultaneously, the only way to know what's active is a single line in the status bar (`[2 active: fix bug]`) and the chat area title (`fix bug [Claude] (1/3)`). Users must memorize which sessions are running and cycle through them blindly with `Ctrl+J/K`. This is the biggest usability gap for power users who regularly run 3-5 concurrent sessions.

A visible tab bar in the chat area makes all active sessions discoverable at a glance, enables mouse-driven switching, and provides a mental model users already know from browsers and tmux.

## User-Visible Outcome

### When this milestone is complete, the user can:

- See a horizontal tab bar at the top of the chat area when one or more PTY sessions are active
- Each tab shows: agent icon (color-coded) + truncated session title + running/done state indicator
- Click a tab with the mouse to switch to that session
- Use existing `Ctrl+J/K` keyboard shortcuts to cycle tabs (unchanged)
- Use `Ctrl+Q` to close the current tab (unchanged, but tab bar updates immediately)
- When no PTY sessions are active, the tab bar is hidden and the placeholder view is shown
- The active tab is visually distinct (highlighted border/background vs dim inactive tabs)

### Entry point / environment

- Entry point: `amux` TUI
- Environment: local terminal
- Live dependencies involved: crossterm mouse capture (new dependency on terminal mouse support)

## Completion Class

- Contract complete means: tab bar renders correctly, click switches tabs, keyboard switching still works, tab bar updates on session spawn/close
- Integration complete means: tab bar coexists with search/filter (M003), scrollback (Page Up/Down), PTY resize, and input forwarding without interference
- Operational complete means: mouse capture is cleanly enabled/disabled with terminal lifecycle; no mouse events leak to PTY subprocess; terminal restored correctly on exit

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- Tab bar appears when first PTY is spawned, shows 1 tab with correct agent icon and title
- Spawning a second session adds a second tab; clicking the first tab switches back
- `Ctrl+J/K` still cycles tabs; tab bar highlights match active_pty
- `Ctrl+Q` closes current tab; tab bar removes it; focus moves to adjacent tab
- Tab bar disappears when all PTYs are closed
- PTY content renders correctly in the reduced-height area (1 row shorter)
- Mouse click events don't interfere with sidebar or status bar
- Existing tests pass, `cargo clippy -- -D warnings` exits 0

## Architectural Decisions

### Tab bar inside chat border

**Decision:** Render the tab bar as the first row inside the chat area's `Block::default().borders(Borders::ALL)`. Use `Layout` to split the block's inner area into `[tab_bar: Length(1)] + [pty_content: Min(1)]`.

**Rationale:** Keeps the tab bar visually contained within the chat panel — same border, same visual scope. No new border or framing needed. Reduces PTY render area by exactly 1 row, which is negligible.

**Alternatives Considered:**
- Separate global tab bar across full width — breaks the sidebar/chat split visual boundary
- Tab bar outside the border — wastes space, looks disconnected from chat content

### Mouse capture via crossterm

**Decision:** Enable `EnableMouseCapture` in `init_terminal()` alongside `EnterAlternateScreen`. Handle `Event::Mouse` in the main event loop. On left-click, check if coordinates fall within a tab's bounding rect; if so, switch `active_pty`.

**Rationale:** Crossterm provides cross-platform mouse support (button events, not just motion). Only left-click on tab bar is handled — all other mouse events are ignored. Mouse capture is disabled in `restore_terminal()`.

**Alternatives Considered:**
- No mouse support, keyboard only — tab bar would be visual-only; misses the click-to-switch value
- Full mouse support (scroll, right-click) — over-scoped; only tab switching needs mouse

### Tab rendering as inline Span list

**Decision:** Render tabs as a single `Line` of `Span`s within a `Paragraph` widget. Each tab is a fixed-width segment (e.g., 20 chars) showing `[C] title...`. Active tab uses yellow bg; inactive uses dark gray. Separator between tabs via `│` character.

**Rationale:** Single-line Paragraph is the simplest ratatui widget. No need for custom widget or Tabs widget (which has more complex state management). Fits naturally into the 1-row layout constraint.

**Alternatives Considered:**
- ratatui `Tabs` widget — designed for this but requires `TabState` and is heavier than needed for a simple single-line display
- Custom widget — over-engineered for a row of styled text segments

### Tab click coordinate mapping

**Decision:** Store the rendered tab bar's `Rect` (from the layout split) on App. On mouse click, check if click.y == tab_rect.y and click.x falls within a tab segment. Tab widths are uniform (or calculated from title length). Map x-offset to tab index.

**Rationale:** Direct coordinate comparison is fast and deterministic. No need for hit-testing framework. The tab bar is exactly 1 row high, so y check is trivial.

**Alternatives Considered:**
- Store per-tab bounding rects — more precise but over-engineered for uniform-width tabs
- Use ratatui's `Rect::intersection` — would work but adds complexity for a simple row

## Error Handling Strategy

No fallible operations in tab rendering. Mouse events that don't hit a tab are silently ignored. Terminal mouse capture failure (rare: headless terminal without mouse support) is non-fatal — tab bar still renders, just not clickable. Log a warning on mouse capture failure if possible.

## Risks and Unknowns

- **Mouse capture compatibility** — Some terminal multiplexers (tmux, screen) may not forward mouse events. Mitigation: keyboard switching (`Ctrl+J/K`) is unaffected; mouse is a convenience, not a requirement.
- **PTY render area resize** — Reducing the PTY area by 1 row requires calling `slot.handle.resize()` with the new dimensions. Must happen after layout calculation. The current code already calls resize before rendering, so this is natural.
- **Tab overflow** — With many concurrent sessions (>10), tabs may exceed the chat area width. Mitigation: truncate tab titles and/or scroll the tab bar. For MVP, truncate with `...` and show at most N tabs that fit.
- **Mouse event forwarding to PTY** — Mouse events must NOT be forwarded to the PTY subprocess as escape sequences. The main event loop must consume all `Event::Mouse` before they could reach PTY input.

## Existing Codebase / Prior Art

- `src/app/ui.rs` — `render_chat()` currently renders the entire chat area as a single Block with PTY content or placeholder. Tab bar will split the inner area. The method already computes `scroll_offset` and title from `active_pty`.
- `src/app/ui.rs` — `render_status()` shows `[N active: current_title]` in the bottom status bar. This can be simplified since tab bar now shows active sessions.
- `src/app/handler.rs` — `Ctrl+J/K` tab switching logic (lines 30-49) and `Ctrl+Q` kill logic (lines 18-28) are already implemented and correct.
- `src/app/mod.rs` — Main event loop (lines 423-443) handles `Event::Key` and `Event::Paste`. Must add `Event::Mouse` handling.
- `src/util.rs` — `init_terminal()` enables raw mode and alternate screen. Must add `EnableMouseCapture`. `restore_terminal()` must add `DisableMouseCapture`.
- `src/types.rs` — `PtySlot` struct has `info: RunningInfo` with `title`, `agent`, `completed` fields — all needed for tab rendering.

## Relevant Requirements

- New requirement: visual tab bar for active PTY sessions
- New requirement: mouse-driven tab switching
- No existing requirements are directly advanced by this work

## Scope

### In Scope

- Enable mouse capture in `init_terminal()`, disable in `restore_terminal()`
- Add `Event::Mouse` handling to main event loop
- Add tab bar rendering in `render_chat()` — 1 row inside chat border
- Tab shows: agent icon (color) + truncated title + state indicator (● running, ✔ done)
- Active tab highlighted (yellow bg), inactive tabs dimmed (dark gray)
- Click on tab → switch `active_pty`
- Tab bar hidden when `ptys` is empty (show existing placeholder)
- Update `render_status()` to simplify (tab bar now shows session info)
- Handle tab overflow: truncate titles to fit available width
- Store tab bar Rect for click coordinate mapping
- Unit tests for tab index calculation from click coordinates

### Out of Scope / Non-Goals

- Drag-to-reorder tabs
- Tab close button (×) — close via Ctrl+Q only
- Right-click context menu on tabs
- Middle-click to close tab
- Tab bar keyboard navigation (separate from Ctrl+J/K)
- Persistent tab ordering or pinned tabs
- Tab bar in sidebar or global position

## Technical Constraints

- Must not break existing keyboard tab switching (Ctrl+J/K)
- Must not break existing Ctrl+Q kill behavior
- Mouse capture must be cleanly toggled with terminal lifecycle
- PTY resize must account for the 1-row tab bar
- `edition = "2024"` Rust
- No new crate dependencies (crossterm mouse support is built-in)

## Integration Points

- `render_chat()` — primary rendering change: split inner area into tab bar + PTY content
- Main event loop in `app/mod.rs` — add `Event::Mouse` dispatch
- `init_terminal()` / `restore_terminal()` in `util.rs` — enable/disable mouse capture
- `Ctrl+J/K` handler — unchanged but must produce correct tab bar highlight
- `Ctrl+Q` handler — unchanged but tab bar must update after removal
- `spawn_with_agent()` — tab bar must update when new PTY added

## Testing Requirements

- Unit tests for tab index calculation (given click x, tab width, number of tabs → expected index)
- Unit tests for tab title truncation (given width and title → truncated string)
- Existing tests must continue passing
- `cargo clippy -- -D warnings` exits 0
- Manual verification: spawn 2+ sessions → see tab bar → click tabs → Ctrl+J/K → Ctrl+Q → tab removed

## Acceptance Criteria

1. Tab bar appears when at least 1 PTY session is active
2. Each tab shows agent icon (color-coded), truncated title, and running/done state
3. Active tab is visually distinct from inactive tabs
4. Clicking an inactive tab switches to it (active_pty updates, PTY content changes)
5. Ctrl+J/K cycling still works and tab bar highlights match
6. Ctrl+Q removes current tab from tab bar and switches to adjacent
7. Tab bar disappears when all PTYs are closed
8. Tab titles truncate with ... when too many tabs for available width
9. No regressions: existing tests pass, clippy clean, fmt clean

## Open Questions

- None — scope is well-defined and bounded
