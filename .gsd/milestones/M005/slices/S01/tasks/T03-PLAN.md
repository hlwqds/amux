---
estimated_steps: 16
estimated_files: 1
skills_used: []
---

# T03: Render tab bar in chat area with layout split

Modify `render_chat()` in `src/app/ui.rs` to:

1. When `ptys` is non-empty, split `block.inner(area)` into `[tab_bar: Length(1)] + [pty_content: Min(1)]` using vertical Layout.
2. Store `self.tab_bar_rect = chunks[0]`.
3. Build tab bar as a `Paragraph` of `Line` containing `Span`s for each tab:
   - Each tab segment: `[AGENT_ICON] title... STATE_INDICATOR `
   - Agent icon in agent color (Agent::color())
   - Title truncated to fit available width per tab
   - State: `●` (running, yellow) or `✔` (done, green)
   - Active tab: highlighted bg (e.g., `Color::Rgb(24, 36, 72)`)
   - Inactive tabs: `Color::DarkGray` fg
   - Separator `│` between tabs
4. Render tab bar Paragraph on `chunks[0]`.
5. Use `chunks[1]` for PTY content instead of `inner` — pass to `slot.handle.resize()` and PseudoTerminal widget.
6. When `ptys` is empty, skip tab bar entirely (existing placeholder path unchanged).

Add helper method `build_tab_bar(&self, width: usize) -> Line<'static>` that constructs the tab Spans.
Add helper `truncate_title(title: &str, max_len: usize) -> String` for tab title truncation.

## Inputs

- `src/app/ui.rs`

## Expected Output

- `src/app/ui.rs`

## Verification

cargo clippy -- -D warnings 2>&1 | tail -5
