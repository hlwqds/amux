# M005 — Research

**Date:** 2026-06-02

## Summary

M005 adds a PTY tab bar to the chat area: a single-line bar of tabs at the top of the chat panel that shows all active PTY sessions with agent icon, title, and state. It enables mouse-driven tab switching alongside existing `Ctrl+J/K` keyboard navigation.

The implementation is straightforward — the codebase already has clean patterns for layout splitting, styled text rendering, and PTY state management. The three changes are: (1) enable/disable mouse capture in `init_terminal()`/`restore_terminal()`, (2) add `Event::Mouse` handling in the main event loop, and (3) split the chat area's inner region into `[tab_bar: 1 row] + [pty_content: Min(1)]` in `render_chat()`.

The primary risk is mouse event leakage to PTY subprocess — the event loop must consume all `Event::Mouse` before they could reach the PTY input path. A secondary concern is tab overflow with many concurrent sessions, which the context already scopes to truncate with `...` for MVP.

## Recommendation

Implement as a single slice. The change touches 4 files with well-defined boundaries: `util.rs` (mouse capture), `app/mod.rs` (event dispatch + `tab_bar_rect` field), `app/ui.rs` (tab bar rendering + click mapping), and `types.rs` (no changes needed — existing types suffice). A second slice for unit tests is unnecessary — tests can live alongside the implementation.

**Build order:** Mouse capture first (unblocks event handling) → event dispatch → tab bar rendering → click coordinate mapping. All can be done in one pass since they're tightly coupled.

## Implementation Landscape

### Key Files

- `src/util.rs` — `init_terminal()` / `restore_terminal()`: add `EnableMouseCapture` / `DisableMouseCapture` to crossterm `execute!` calls alongside existing `EnterAlternateScreen` / `LeaveAlternateScreen`
- `src/app/mod.rs` — Main event loop (line ~440): add `Event::Mouse` arm. Add `tab_bar_rect: Rect` field to `App` struct for storing the rendered tab bar position. Add `handle_mouse_click(&mut self, x: u16, y: u16)` method.
- `src/app/ui.rs` — `render_chat()`: split `block.inner(area)` into `[tab_bar: Length(1)] + [pty_content: Min(1)]`. Render tabs as a `Paragraph` of `Span`s. Store tab bar rect in `self.tab_bar_rect`. When `ptys` is empty, hide tab bar (placeholder view unchanged).
- `src/app/handler.rs` — No changes needed. `Ctrl+J/K` cycling and `Ctrl+Q` kill already work correctly. Tab bar reads from `active_pty` which they already update.
- `src/types.rs` — No changes needed. `PtySlot`, `RunningInfo`, `Agent` (icon/color), and `PtyState` already provide everything needed for tab rendering.

### Existing Patterns to Follow

- **Layout splitting:** The sidebar already splits into `[tree: Min(3)] + [search: Length(1)]` during search mode (lines 198-203 of `ui.rs`). Same pattern for tab bar: `[tab_bar: Length(1)] + [pty: Min(1)]`.
- **Styled Span lists:** The sidebar renders `ListItem`s with `Vec<Span>` for agent icons, colored dots, etc. Tab bar uses the same approach but in a `Paragraph` (single `Line` of `Span`s).
- **Agent color/icon:** `Agent::icon()` returns `"C"`/`"X"`/`"G"`, `Agent::color()` returns `Color::Cyan`/`Color::Green`/`Color::Magenta`. Already used throughout the sidebar.
- **PTY state:** `PtyState::Running` / `PtyState::Completed` already computed via `slot.handle.state()`. Sidebar shows `●` (running, yellow) and `✔` (done, green) — reuse same indicators.

### Crossterm Mouse API

```rust
// In util.rs init_terminal():
use crossterm::event::{EnableMouseCapture, DisableMouseCapture};
execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

// In util.rs restore_terminal():
execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;

// In event loop:
Event::Mouse(mouse_event) => {
    match mouse_event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            app.handle_mouse_click(mouse_event.column, mouse_event.row);
        }
        _ => {} // Ignore all other mouse events
    }
}
```

`Event::Mouse` carries `MouseEvent { kind, column, row }`. Only `MouseEventKind::Down(MouseButton::Left)` needs handling — all other mouse events (move, scroll, release, right-click) are silently consumed (not forwarded to PTY).

### Tab Bar Rendering Design

```rust
// In render_chat(), after computing block:
let inner = block.inner(area);
let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(1), Constraint::Min(1)])
    .split(inner);

// chunks[0] = tab bar (1 row)
// chunks[1] = PTY content

// Store tab bar rect for click mapping
self.tab_bar_rect = chunks[0];

// Render tab bar only when ptys.len() > 0
if !self.ptys.is_empty() {
    let tabs = self.build_tab_bar(chunks[0].width as usize);
    frame.render_widget(Paragraph::new(tabs), chunks[0]);
}

// PTY uses chunks[1] instead of inner
let pty_area = chunks[1];
slot.handle.resize((pty_area.width, pty_area.height));
```

Each tab is a fixed-width segment: `[C] title... │` where:
- `C` = agent icon in agent color
- `title...` = truncated title (max ~15 chars)
- Active tab: `Style::default().bg(Color::Rgb(24, 36, 72))` (same highlight as sidebar)
- Inactive tab: `Style::default().fg(Color::DarkGray)`
- State indicator: `●` (running, yellow) or `✔` (done, green) — same as sidebar
- Separator: `│` between tabs

### Click Coordinate Mapping

```rust
fn handle_mouse_click(&mut self, x: u16, y: u16) {
    // Check if click is within tab bar
    if y < self.tab_bar_rect.top()
        || y >= self.tab_bar_rect.bottom()
        || x < self.tab_bar_rect.left()
        || x >= self.tab_bar_rect.right()
    {
        return; // Click outside tab bar — ignore
    }

    let local_x = x - self.tab_bar_rect.left();
    let tab_width = self.calculate_tab_width(self.tab_bar_rect.width as usize);
    if tab_width == 0 { return; }

    let tab_index = (local_x as usize) / tab_width;
    if tab_index < self.ptys.len() && tab_index != self.active_pty.unwrap_or(usize::MAX) {
        self.active_pty = Some(tab_index);
        if let Some(s) = self.ptys.get(tab_index) {
            s.handle.reset_scroll();
        }
        self.status = format!(
            "Switched to: {} ({}/{})",
            self.ptys[tab_index].info.title,
            tab_index + 1,
            self.ptys.len()
        );
    }
}
```

### PTY Area Resize Impact

Currently `render_chat()` calls `slot.handle.resize((inner.width, inner.height))` where `inner = block.inner(area)`. With the tab bar, `inner` is further split. The PTY area becomes `chunks[1]` which is 1 row shorter. This is handled naturally by passing `chunks[1]` dimensions to resize. The `chat_size()` method uses `last_chat_area` to compute size — this already works correctly since `last_chat_area` is set to `cols[1]` (the chat panel area) and `chat_size()` subtracts 2 for borders. The tab bar further reduces available height by 1, which happens inside the render function via the layout split.

### Status Bar Simplification

The status bar currently shows `[N active: current_title]` when PTYs are running. With the tab bar showing this info, the status bar's PTY section can be simplified to just `[N active]` or removed entirely. The context says "Update render_status() to simplify" but this is a minor cosmetic change — the tab bar is the primary session info display.

## Constraints

- **No new crate dependencies:** crossterm's `EnableMouseCapture`/`DisableMouseCapture` are built-in. No need for additional crates.
- **Mouse events must not leak to PTY:** The event loop currently has `Event::Key` and `Event::Paste` arms. Adding `Event::Mouse` must consume all mouse events — the `_ => {}` catch-all in the event loop's match currently ignores unknown events, but `Event::Mouse` must be explicitly handled before it.
- **Edition 2024 Rust:** Uses `let` chains in `if let` (already seen in codebase).
- **Tab bar inside chat border:** The block title still shows session info (`title [Agent] (1/3)`). The tab bar goes inside the block border, as the first row of the inner area. This means the block title and the tab bar coexist — title at top of border, tabs inside.

## Common Pitfalls

- **Mouse event forwarding to PTY:** The `_ => {}` arm in the event loop must not fall through to the PTY input path. Currently the event loop dispatches on event type and the key handler has the PTY write path. Mouse events go to a separate `Event::Mouse` arm, so they won't accidentally reach `slot.handle.write_input()`. But double-check that the match is exhaustive.
- **Tab bar rect not initialized:** `tab_bar_rect` defaults to `Rect::default()` (0,0,0,0). If a click happens before the first render, the y-coordinate check would fail (0 >= 0 is false for bottom). This is fine — clicks before render are harmless.
- **Terminal multiplexer mouse support:** tmux/screen may not forward mouse events. Keyboard switching (`Ctrl+J/K`) is unaffected. Mouse is a convenience, not a requirement. Non-fatal if mouse capture fails — consider wrapping in `if let Ok(...)` or just logging.
- **Tab overflow with many sessions:** With 5+ concurrent sessions, tabs may exceed chat area width. MVP: truncate with `...`, show only tabs that fit. Implementation: calculate max tabs that fit in `tab_bar_rect.width`, only render those. Mark non-fitting tabs with a `»` indicator at the end.
- **PseudoTerminal block ownership:** Currently `PseudoTerminal::new(&screen).block(block)` owns the block. After splitting, the block is rendered once, and the PTY uses `chunks[1]` as its area. The block must wrap the entire area (including tab bar), and the PTY renders inside `chunks[1]`. This means the block's `.inner()` is split, not the PTY's own block.

## Open Risks

- **Mouse capture compatibility in CI/headless:** Tests that create terminals may fail if mouse capture is required. Mitigation: mouse capture is in `init_terminal()` which is only called when `stdout().is_terminal()` returns true.
- **tui-term PseudoTerminal + layout interaction:** The `PseudoTerminal` widget currently receives the full area with block. After tab bar split, it needs just `chunks[1]` without a separate block. The `.block(block)` call on PseudoTerminal means it draws its own border. The tab bar renders into `chunks[0]` (the first row of `block.inner(area)`). This should work because `PseudoTerminal::new(&screen).block(block)` renders the block border and then the PTY content inside it — but we need the PTY to not cover the tab bar area. Solution: render the block first, get inner, split inner, render tab bar into chunks[0], render PTY into chunks[1] **without** a block (since the outer block already provides the border). This requires using `PseudoTerminal::new(&screen)` without `.block()` and rendering the block separately.

## Sources

- Crossterm mouse API: `crossterm::event::{EnableMouseCapture, DisableMouseCapture, Event::Mouse, MouseEvent, MouseEventKind, MouseButton}` — confirmed via Context7 docs for crossterm 0.29
- ratatui `Paragraph` + `Line` + `Span` pattern: already used extensively in sidebar rendering
- Layout split pattern: `src/app/ui.rs` lines 198-203 (sidebar search split)
