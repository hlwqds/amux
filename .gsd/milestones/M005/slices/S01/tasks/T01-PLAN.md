---
estimated_steps: 9
estimated_files: 1
skills_used: []
---

# T01: Enable mouse capture in terminal lifecycle

Add `EnableMouseCapture` to `init_terminal()` execute! macro alongside `EnterAlternateScreen`. Add `DisableMouseCapture` to `restore_terminal()` execute! macro alongside `LeaveAlternateScreen`. Import from `crossterm::event`.

```rust
// init_terminal():
use crossterm::event::EnableMouseCapture;
execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

// restore_terminal():
use crossterm::event::DisableMouseCapture;
execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
```

## Inputs

- `src/util.rs`

## Expected Output

- `src/util.rs`

## Verification

cargo clippy -- -D warnings 2>&1 | tail -5
