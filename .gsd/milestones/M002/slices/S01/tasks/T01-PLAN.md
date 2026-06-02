---
estimated_steps: 10
estimated_files: 2
skills_used: []
---

# T01: Create lib.rs and rewire main.rs for lib/bin split

Create src/lib.rs with: `pub mod app; pub mod config; pub mod discovery; pub mod pty; pub mod types; pub mod util;`
In src/main.rs, remove all `mod` declarations (mod app; mod config; mod discovery; mod pty; mod types; mod util;)
Add `use amux::app;` at the top of main.rs
In the #[cfg(test)] mod tests block, change:
- `use super::config::*;` → `use amux::config::*;`
- `use super::discovery::*;` → `use amux::discovery::*;`
- `use super::types::*;` → `use amux::types::*;`
- `use super::util::*;` → `use amux::util::*;`

Constraints: Do NOT move or modify any test bodies. Do NOT change app.rs, config.rs, discovery.rs, pty.rs, types.rs, or util.rs.

Done when: cargo build succeeds, cargo test shows 33 passed, and src/lib.rs exists as a valid library root.

## Inputs

- `src/main.rs`
- `src/app.rs`
- `src/config.rs`
- `src/discovery.rs`
- `src/pty.rs`
- `src/types.rs`
- `src/util.rs`
- `Cargo.toml`

## Expected Output

- `src/lib.rs`
- `src/main.rs`

## Verification

cargo test
