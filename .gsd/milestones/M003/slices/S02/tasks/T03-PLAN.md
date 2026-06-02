---
estimated_steps: 6
estimated_files: 1
skills_used: []
---

# T03: Render combined filter indicators in sidebar header

1. In `src/app/ui.rs`, update `render_sidebar()` block title:
   - Show `[search: query]` when text search is active
   - Show `[Claude/Codex/GSD]` when agent filter is active
   - Show both when both are active
   - Show plain `Workspaces` when no filters active
2. Run `cargo check`

## Inputs

- `src/app/ui.rs`

## Expected Output

- `ui.rs with combined filter header rendering`

## Verification

cargo check
