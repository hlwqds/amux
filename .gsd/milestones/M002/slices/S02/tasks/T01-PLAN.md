---
estimated_steps: 17
estimated_files: 6
skills_used: []
---

# T01: Create app/ directory and extract sub-modules

Why: app.rs is a 1558-line God Object that must be split into focused sub-modules for maintainability.

Do:
1. Create src/app/ directory and move src/app.rs to src/app/mod.rs (zero content changes).
2. Compile to verify the move works (cargo build).
3. Extract app/ui.rs: Cut the 8 render_* methods (lines 925-1491) from mod.rs into a new app/ui.rs file wrapped in `impl App { ... }`. Add the necessary imports (ratatui, crate::pty::PtyState, crate::types::*, crate::util::{relative_time, centered_rect}). Add `mod ui;` to mod.rs. Compile.
4. Extract app/handler.rs: Cut the 5 handle_* methods (handle_key 219-391, handle_input_key 393-421, handle_browse_key 423-445, handle_agent_key 447-509, handle_paste 585-595) from mod.rs. Add imports (crossterm event, crate::types::*, crate::util::key_to_bytes, anyhow::Result). Add `mod handler;` to mod.rs. Compile.
5. Extract app/session.rs: Cut spawn_session (511-514), spawn_with_agent (516-583), confirm_input (597-675), start_rename (677-693), start_new_workspace (695-700). Add imports (std::env, anyhow, crate::config::*, crate::pty::PtyHandle, crate::types::*, crate::util::*). Add `mod session;` to mod.rs. Compile.
6. Extract app/browse.rs: Cut start_browse_dir (702-708), load_browse_entries (710-754), browse_move (756-764), browse_select (766-820), browse_up (822-827). Add imports (std::env, std::fs, std::path::PathBuf, crate::config::*, crate::types::*, crate::util::*). Add `mod browse;` to mod.rs. Compile.
7. Remove unused imports from app/mod.rs (ratatui, tui_term, crossterm KeyCode/KeyModifiers that moved to sub-modules). Compile.

Key constraints:
- All methods remain `fn` (not pub fn) — child modules access private App fields natively.
- No trait indirection needed — all methods are `impl App` blocks.
- mod declarations in mod.rs must appear before any `impl App` block.
- Compile after each extraction step to catch borrow/visibility issues immediately.
- The extraction order (ui → handler → session → browse) follows biggest-first to surface issues early.
- confirm_input lives in session.rs even though it calls start_browse_dir (in browse.rs) — cross-module impl calls are fine in Rust.

Done when: cargo build succeeds with app/ directory structure in place and app.rs deleted.

## Inputs

- `src/app.rs`

## Expected Output

- `src/app/mod.rs`
- `src/app/ui.rs`
- `src/app/handler.rs`
- `src/app/session.rs`
- `src/app/browse.rs`

## Verification

cargo build
