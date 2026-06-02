# S02 Research — app.rs decomposition into sub-modules

**Date:** 2026-06-02

## Summary

app.rs (1558 lines) contains a single private `struct App` with 23 fields and an `impl App` block with 37 methods, plus a standalone `pub fn run()`. The decomposition follows Rust's standard idiom: multiple `impl App` blocks across files within the same module tree. The app directory structure (`src/app/mod.rs` + sub-modules) gives child modules access to App's private fields. All cross-method calls are within `self.*` — no trait indirection needed.

## Key Findings

### 1. Visibility is safe — no changes needed

- `struct App` is **private** (no `pub`). In Rust, child modules of `app` (i.e., `app::ui`, `app::handler`) can access private items of their parent. So `app/ui.rs` can freely read/write `self.workspaces`, `self.sessions`, `self.ptys`, etc. **No field visibility changes needed.**
- `pub fn run()` is the only public entry point. It stays in `app/mod.rs`.

### 2. Cross-module call graph

Handler → Session/Browse calls from handler:
- `handle_input_key` → `handle_browse_key`, `handle_agent_key` (dispatch)
- `handle_browse_key` → `self.browse_move()`, `self.browse_select()`, `self.browse_up()`
- `handle_agent_key` → `self.confirm_input()`
- `handle_key` → `self.start_rename()`, `self.start_new_workspace()`, `self.delete_selected()`, `self.activate_selection()`, `self.refresh_sessions()`

Session → Browse calls:
- `confirm_input()` (session) calls `self.start_browse_dir()` (browse) — this is a **cross-sub-module call** from session to browse. This is fine: both are `impl App` methods, just in different files. No adapter needed.

Core → Session calls:
- `activate_selection()` (core) calls `self.spawn_with_agent()` (session)
- `delete_selected()` (core) accesses `self.ptys`, `self.pty_index_for_session()`

### 3. Import requirements per sub-module

**app/mod.rs** (core):
```rust
use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Context, Result};
use crate::config::{load_config, save_config_file, data_dir, title_override_path, save_session_title, generate_id};
use crate::discovery::{discover_sessions, find_session_jsonl};
use crate::pty::{PtyHandle, PtyState};
use crate::types::*;
use crate::util::*;
use crossterm::event::{Event, KeyEvent, KeyEventKind};
```

**app/ui.rs**:
```rust
use ratatui::{Frame, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap}};
use tui_term::widget::PseudoTerminal;
use crate::pty::PtyState;
use crate::types::*;
use crate::util::{relative_time, centered_rect};
```

**app/handler.rs**:
```rust
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::types::*;
use crate::util::key_to_bytes;
```

**app/session.rs**:
```rust
use std::env;
use anyhow::{Context, Result};
use crate::config::{generate_id, save_config_file, save_session_title, title_override_path, data_dir};
use crate::pty::PtyHandle;
use crate::types::*;
use crate::util::{detect_agents, now_secs};
```

**app/browse.rs**:
```rust
use std::env;
use std::fs;
use std::path::PathBuf;
use crate::config::{generate_id, save_config_file};
use crate::types::*;
```

### 4. Method → file assignment (refined from research)

**app/mod.rs — Core (~300 lines):**
| Method | Lines | Notes |
|--------|-------|-------|
| `struct App` definition | 26-50 (~24 lines) | All 23 fields |
| `new()` | 53-98 (~46 lines) | Constructor, calls `rebuild_tree()` |
| `poll_states()` | 100-131 (~32 lines) | PTY state polling, calls `rebuild_tree()` |
| `pty_display_state()` | 133-139 (~7 lines) | Helper |
| `refresh_sessions()` | 141-155 (~15 lines) | Session discovery |
| `rebuild_tree()` | 157-189 (~33 lines) | Tree construction |
| `pty_index_for_session()` | 191-195 (~5 lines) | Lookup |
| `selected_node()` | 197-199 (~3 lines) | Accessor |
| `move_sel()` | 201-209 (~9 lines) | Selection movement |
| `toggle_expand()` | 211-217 (~7 lines) | Expand/collapse |
| `delete_selected()` | 829-867 (~39 lines) | Delete workspace/session |
| `save_config()` | 869-876 (~8 lines) | Config save |
| `workspace_cwd()` | 878-887 (~10 lines) | Path helper |
| `ws_matches_path()` | 889-894 (~6 lines) | Path matcher |
| `activate_selection()` | 896-914 (~19 lines) | Enter key action, calls `spawn_with_agent` |
| `chat_size()` | 916-923 (~8 lines) | Layout helper |
| `run()` | 1493-1558 (~66 lines) | Public entry point, terminal loop |
| mod declarations | 3-5 lines | `mod ui; mod handler; mod session; mod browse;` |

**app/ui.rs — Rendering (~530 lines):**
| Method | Lines | Notes |
|--------|-------|-------|
| `render()` | 925-946 (~22 lines) | Top-level, calls sub-renderers |
| `render_sidebar()` | 948-1104 (~157 lines) | Sidebar with tree nodes |
| `render_chat()` | 1106-1166 (~61 lines) | Chat pane with PTY |
| `render_placeholder()` | 1168-1277 (~110 lines) | Placeholder when no PTY |
| `render_input_popup()` | 1279-1319 (~41 lines) | Input dialog |
| `render_agent_popup()` | 1321-1369 (~49 lines) | Agent selection |
| `render_browse_popup()` | 1371-1448 (~78 lines) | Directory browser |
| `render_status()` | 1450-1491 (~42 lines) | Status bar |

**app/handler.rs — Key handling (~370 lines):**
| Method | Lines | Notes |
|--------|-------|-------|
| `handle_key()` | 219-391 (~173 lines) | Main dispatcher |
| `handle_input_key()` | 393-421 (~29 lines) | Input mode dispatch |
| `handle_browse_key()` | 423-445 (~23 lines) | Browse mode keys |
| `handle_agent_key()` | 447-509 (~63 lines) | Agent selection keys |
| `handle_paste()` | 585-595 (~11 lines) | Paste handling |

**app/session.rs — Session management (~180 lines):**
| Method | Lines | Notes |
|--------|-------|-------|
| `spawn_session()` | 511-514 (~4 lines) | Entry point |
| `spawn_with_agent()` | 516-583 (~68 lines) | Full spawn logic |
| `confirm_input()` | 597-675 (~79 lines) | Input confirmation dispatch |
| `start_rename()` | 677-693 (~17 lines) | Rename initiation |
| `start_new_workspace()` | 695-700 (~6 lines) | Workspace creation |

**app/browse.rs — Directory browsing (~125 lines):**
| Method | Lines | Notes |
|--------|-------|-------|
| `start_browse_dir()` | 702-708 (~7 lines) | Browse initiation |
| `load_browse_entries()` | 710-754 (~45 lines) | Directory listing |
| `browse_move()` | 756-764 (~9 lines) | Selection movement |
| `browse_select()` | 766-820 (~55 lines) | Selection action |
| `browse_up()` | 822-827 (~6 lines) | Navigate up |

### 5. Gotchas and risks

1. **`centered_rect` is in util.rs** — UI methods call it via `crate::util::*` which is already imported. After decomposition, `app/ui.rs` needs `use crate::util::centered_rect;` (or `use crate::util::*`).

2. **`std::io::IsTerminal`** — used only in `run()` (app/mod.rs), not in sub-modules.

3. **`run()` uses crossterm events directly** — `Event::Key`, `Event::Paste` are in the event loop in `run()`. Only `run()` and handler methods need crossterm imports.

4. **`confirm_input()` spans session AND browse** — It dispatches to `start_browse_dir()` for `InputMode::NewWorkspaceName`. This cross-module call is fine in Rust (both are `impl App` methods), but confirm_input logically belongs in session.rs since it primarily handles session-related input modes (SessionName, RenameSession, SelectAgent, NewWorkspaceName, RenameWorkspace).

5. **`handle_key` calls methods from all groups** — `handle_key` (handler) calls `refresh_sessions`, `move_sel`, `toggle_expand`, `start_rename`, `start_new_workspace`, `delete_selected`, `activate_selection` — methods that live in core/mod.rs, session.rs, and browse.rs. These cross-module calls work naturally because all are `impl App` methods.

6. **No new `pub` visibility needed** — Since `App` is private to the `app` module and sub-modules are children, all `impl App` methods can remain `fn` (not `pub fn`). The Rust compiler allows child modules to add `impl` blocks for parent-private types.

7. **Edition 2024 `let` chains** — The code uses `if let Some(x) = expr && condition { }` syntax (edition 2024 feature). This compiles fine and doesn't affect decomposition.

### 6. Build order for extraction

The safest extraction order — compile after each step:

1. **Create `src/app/` directory**, move `src/app.rs` → `src/app/mod.rs` (zero changes to file content). Compile to verify.

2. **Extract `ui.rs`** — Cut the 8 render_* methods from `mod.rs`, add imports, create `app/ui.rs` with `impl App { ... }`. Add `mod ui;` to `mod.rs`. Compile.

3. **Extract `handler.rs`** — Cut the 5 handle_* methods, add imports, create `app/handler.rs`. Add `mod handler;` to `mod.rs`. Compile.

4. **Extract `session.rs`** — Cut spawn_*, confirm_input, start_rename, start_new_workspace. Add imports, create `app/session.rs`. Add `mod session;` to `mod.rs`. Compile.

5. **Extract `browse.rs`** — Cut browse_* methods, start_browse_dir, load_browse_entries. Add imports, create `app/browse.rs`. Add `mod browse;` to `mod.rs`. Compile.

6. **Final cleanup** — Remove unused imports from `mod.rs`, verify all 33 tests pass, run clippy and fmt.

### 7. Constants and external dependencies

- `SELECT_CURRENT`, `SELECT_VIRTUAL`, `PARENT_DIR` are in `util.rs` (not types.rs) — browse module needs `use crate::util::*` or specific imports.
- `centered_rect` is in `util.rs` — UI module needs it.
- `detect_agents` and `key_to_bytes` are in `util.rs` — handler and core need them.
- `init_terminal` and `restore_terminal` are in `util.rs` — only `run()` in mod.rs uses these.

## Recommendation

**Two tasks:**

1. **T01: Create app/ directory and extract sub-modules** — The mechanical file split. Create `app/mod.rs` from current `app.rs`, then extract methods into 4 sub-modules one at a time, compiling after each. The extraction order (ui → handler → session → browse) follows the "biggest first" principle to surface issues early.

2. **T02: Verify and clean up** — Run cargo test (33 tests), cargo clippy, cargo fmt. Remove unused imports from each file. Verify the binary builds.

The key risk is **borrow checker friction** — unlikely since all methods take `&mut self` independently, but the extraction must be done incrementally with compilation after each step.

## Implementation Landscape

### Key Files
- `src/app.rs` (1558 lines) → split into `src/app/mod.rs` + 4 sub-modules
- `src/app/mod.rs` (~300 lines) — App struct, core methods, run(), mod declarations
- `src/app/ui.rs` (~530 lines) — 8 render_* methods
- `src/app/handler.rs` (~370 lines) — 5 handle_* methods
- `src/app/session.rs` (~180 lines) — spawn, confirm, rename, new workspace
- `src/app/browse.rs` (~125 lines) — directory browsing
- `src/lib.rs` — already exists from S01, no changes needed
- `src/main.rs` — no changes needed

### Verification Commands
```bash
cargo build                    # must compile
cargo test                     # 33 passed, 0 failed
cargo clippy -- -D warnings    # exit 0
cargo fmt --all -- --check     # exit 0
```

### Forward Intelligence
- **Fragility:** The `confirm_input` method is the cross-cutting method — it dispatches across session and browse concerns. If future work adds new input modes, this method will grow. Consider splitting it further if it exceeds ~100 lines after new modes.
- **Changed assumptions:** None. The decomposition preserves all existing semantics.
- **Watch-outs:** After extraction, `app/mod.rs` must declare `mod ui; mod handler; mod session; mod browse;` **before** the `impl App` block, or the compiler won't find the sub-modules. Also, `use super::*` is NOT needed — sub-modules access `App` directly since it's defined in the parent module.
