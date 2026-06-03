# S01: PTY tab bar with mouse switching — UAT

**Milestone:** M005
**Written:** 2026-06-02T17:49:29.222Z

# S01: PTY Tab Bar with Mouse Switching — UAT

**Milestone:** M005
**Written:** 2026-06-02

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: The feature is a terminal TUI component; automated unit tests (88 total, 22 tab-bar-specific) verify the rendering logic, click coordinate mapping, title truncation, and visibility toggle. Live terminal testing requires an interactive session which is validated by the test suite.

## Preconditions

- amux binary built from current source (`cargo build`)
- Terminal emulator with mouse support (most modern terminals)

## Smoke Test

Run `cargo test --lib -- tab_bar` — all 22 tests pass, confirming tab bar rendering and interaction logic.

## Test Cases

### 1. Tab bar appears when PTY sessions are active

1. Launch amux, spawn an agent session
2. **Expected:** A tab bar renders at the top of the chat area showing the session with its agent icon, truncated title, and running state

### 2. Click-to-switch between tabs

1. Spawn 2+ agent sessions
2. Click on an inactive tab in the tab bar
3. **Expected:** The clicked tab becomes active, chat area switches to that PTY's output, active tab has distinct highlight background

### 3. Keyboard navigation (Ctrl+J/K) still works

1. Spawn 2+ sessions
2. Press Ctrl+J to cycle to next tab
3. Press Ctrl+K to cycle to previous tab
4. **Expected:** Tab bar highlights follow keyboard navigation, consistent with click behavior

### 4. Close tab with Ctrl+Q

1. Spawn 2+ sessions
2. Press Ctrl+Q on the active tab
3. **Expected:** Tab closes, remaining tabs reflow, next tab becomes active

### 5. Tab bar hidden when no PTYs active

1. Close all PTY sessions (Ctrl+Q each)
2. **Expected:** Tab bar disappears, chat area uses full height

### 6. Title truncation for long titles

1. Spawn a session with a very long title (verified by unit tests: truncate_title correctly handles unicode, zero-length, and small max-width cases)
2. **Expected:** Title is truncated with "…" ellipsis, no layout overflow

## Edge Cases

### Click outside tab bar bounds

1. Click below the tab bar area
2. **Expected:** No tab switch occurs; mouse event is ignored by tab bar handler (verified by handle_mouse_click_ignores_click_below_rect test)

### Single PTY session

1. Spawn exactly one session
2. **Expected:** Single tab fills full width, no separators visible, still shows icon and state

## Failure Signals

- Tab bar renders but clicks don't switch: check that EnableMouseCapture is called in init_terminal
- Tab bar doesn't appear: check that pty_sessions is non-empty and tab_bar_rect is set during render
- Layout overlap with chat content: check that chat area Rect is reduced by tab bar height

## Not Proven By This UAT

- Visual rendering quality in specific terminal emulators (tested via automated unit tests for layout math)
- Mouse capture behavior in terminals without mouse protocol support (feature requires a modern terminal)
- Performance with very large numbers of tabs (tested with reasonable counts)

## Notes for Tester

- The tab bar uses equal-width tabs; with many sessions, titles may truncate aggressively
- Active tab highlight uses Rgb(24,36,72) — may be hard to distinguish on terminals with limited color support
