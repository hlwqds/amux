---
estimated_steps: 17
estimated_files: 5
skills_used: []
---

# T01: Migrate all 33 tests to per-module locations and strip main.rs

Why: Tests currently live in a single monolithic main.rs::tests block (lines 8–454). Moving them to per-module test blocks colocates tests with the code they test, following Rust convention and completing the modular refactor.

Do:
1. Read src/main.rs and extract all 33 test functions with their bodies verbatim.
2. Append #[cfg(test)] mod tests blocks to each target module:
   - src/config.rs: Add 8 tests (encode_project_path_simple, encode_project_path_root, encode_project_path_relative, generate_id_is_unique, gsd_directory_name_encoding, encode_decode_gsd_dir_roundtrip, decode_gsd_dir_name_simple, decode_gsd_dir_name_root). Import block: use std::path::{Path, PathBuf}; use super::*;
   - src/types.rs: Add 6 tests (config_roundtrip, workspace_serialization_virtual, agent_traits, gsd_sessions_persist_after_pty_exit, gsd_build_new_cmd_no_session_name, gsd_build_resume_cmd_uses_sessions). Import block: use std::path::{Path, PathBuf}; use ratatui::style::Color; use serde_json; use super::*;
   - src/discovery.rs: Add 18 tests (clean_user_message_normal, clean_user_message_escapes, clean_user_message_noise_prefix, clean_user_message_strips_whitespace, extract_text_from_string_content, extract_text_from_array_content, extract_text_from_array_with_non_text, extract_text_from_empty_array, extract_text_from_number, parse_codex_session_valid, parse_codex_session_invalid_json, parse_gsd_session_valid_with_gsd_run_title, parse_gsd_session_fallback_to_user_message, parse_gsd_session_gsd_run_takes_priority, parse_gsd_session_no_session_line, parse_gsd_session_title_truncated_to_50_chars, parse_gsd_session_empty_file, discover_gsd_sessions_finds_by_workspace). Import block: use std::path::{Path, PathBuf}; use crate::config::encode_project_path; use serde_json; use super::*;
   - src/util.rs: Add 1 test (relative_time_formatting). Import block: use super::*;
3. Strip src/main.rs to exactly:
   use amux::app;
   
   fn main() -> anyhow::Result<()> {
       app::run()
   }
   (5 lines, no test module, no comment separator)
4. Copy test bodies verbatim — no logic changes. Only the import block at the top of each mod tests changes (use super::* for module items, plus any extra imports listed above).

Done when: cargo test shows 33 passed from config::tests, types::tests, discovery::tests, util::tests. main.rs has 0 #[test] attributes. cargo clippy and cargo fmt are clean.

## Inputs

- `src/main.rs`
- `src/config.rs`
- `src/types.rs`
- `src/discovery.rs`
- `src/util.rs`

## Expected Output

- `src/main.rs`
- `src/config.rs`
- `src/types.rs`
- `src/discovery.rs`
- `src/util.rs`

## Verification

cargo test --workspace
