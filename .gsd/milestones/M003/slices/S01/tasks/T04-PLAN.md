---
estimated_steps: 5
estimated_files: 1
skills_used: []
---

# T04: Render search prompt and filter indicator in sidebar

1. In `src/app/ui.rs`, modify `render_sidebar()`:
   - When `input_mode == InputMode::Search`, render a search input line at the bottom of the sidebar area showing the prompt with current input_buffer content
   - Update the sidebar block title to show `[search: {query}]` when search is active
2. Ensure search prompt does not overlap with tree items
3. Run `cargo check`

## Inputs

- `src/app/ui.rs`

## Expected Output

- `ui.rs with search prompt rendering`

## Verification

cargo check
