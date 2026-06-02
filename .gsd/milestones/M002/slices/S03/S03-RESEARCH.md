# S03 — Research: Test migration to per-module locations

**Date:** 2026-06-02

## Summary

33 tests currently live in `src/main.rs::tests` (lines 9–454). They all import from the `amux` library crate (`use amux::{config,discovery,types,util}::*`). After migration, main.rs drops to 5 lines (use + fn main + empty line). Each target module gets a `#[cfg(test)] mod tests` block at its end with `use super::*;` for same-module access, plus any cross-module imports needed. All tested functions are `pub` — no visibility changes required. Zero risk of borrow checker or compilation issues; this is purely mechanical test relocation.

## Recommendation

**Single task:** Move all 33 tests from main.rs to per-module test blocks, then strip main.rs to entry-point-only. This is mechanical work with no ambiguity. No need for multi-task decomposition — do it in one pass, verify at the end.

The work order doesn't matter (all modules are independent), but the natural order is: config.rs → types.rs → discovery.rs → util.rs → main.rs cleanup.

## Implementation Landscape

### Current State

- `src/main.rs`: 5 lines of real code + 1 comment separator + 445 lines of tests (33 tests total)
- No module has an existing `#[cfg(test)]` block
- All 33 tested functions/items are `pub` — no visibility changes needed
- Tests already import from `amux::` crate namespace (established in S01)

### Target State

- `src/main.rs`: 5 lines only (use + fn main)
- `src/config.rs`: +8 tests appended as `#[cfg(test)] mod tests`
- `src/types.rs`: +6 tests appended as `#[cfg(test)] mod tests`
- `src/discovery.rs`: +18 tests appended as `#[cfg(test)] mod tests`
- `src/util.rs`: +1 test appended as `#[cfg(test)] mod tests`

### Test Migration Map (33 tests → 4 modules)

**config.rs (8 tests):**
| Test | Notes |
|------|-------|
| `encode_project_path_simple` | Pure config function test |
| `encode_project_path_root` | Pure config function test |
| `encode_project_path_relative` | Pure config function test |
| `generate_id_is_unique` | Pure config function test |
| `gsd_directory_name_encoding` | Tests encode_project_path |
| `encode_decode_gsd_dir_roundtrip` | Tests encode_project_path |
| `decode_gsd_dir_name_simple` | Tests encode_project_path logic |
| `decode_gsd_dir_name_root` | Tests encode_project_path logic |

**types.rs (6 tests):**
| Test | Notes |
|------|-------|
| `config_roundtrip` | Tests Config + Workspace serialization — needs `use serde_json` (serde is already a dep, just need the explicit import in test module) |
| `workspace_serialization_virtual` | Tests Workspace serialization — needs `use serde_json` |
| `agent_traits` | Tests Agent methods — needs `use ratatui::style::Color` |
| `gsd_sessions_persist_after_pty_exit` | Tests Agent-based retain logic concept |
| `gsd_build_new_cmd_no_session_name` | Tests Agent::Gsd.build_new_cmd — needs `use std::path::Path` |
| `gsd_build_resume_cmd_uses_sessions` | Tests Agent::Gsd.build_resume_cmd — needs `use std::path::Path` |

**discovery.rs (18 tests):**
| Test | Notes |
|------|-------|
| `clean_user_message_normal` | Pure discovery function test |
| `clean_user_message_escapes` | Pure discovery function test |
| `clean_user_message_noise_prefix` | Pure discovery function test |
| `clean_user_message_strips_whitespace` | Pure discovery function test |
| `extract_text_from_string_content` | Tests extract_text_from_content — needs `use serde_json` |
| `extract_text_from_array_content` | Tests extract_text_from_content — needs `use serde_json` |
| `extract_text_from_array_with_non_text` | Tests extract_text_from_content — needs `use serde_json` |
| `extract_text_from_empty_array` | Tests extract_text_from_content — needs `use serde_json` |
| `extract_text_from_number` | Tests extract_text_from_content — needs `use serde_json` |
| `parse_codex_session_valid` | Tests parse_codex_session |
| `parse_codex_session_invalid_json` | Tests parse_codex_session |
| `parse_gsd_session_valid_with_gsd_run_title` | Tests parse_gsd_session — needs `use serde_json` |
| `parse_gsd_session_fallback_to_user_message` | Tests parse_gsd_session — needs `use serde_json` |
| `parse_gsd_session_gsd_run_takes_priority` | Tests parse_gsd_session — needs `use serde_json` |
| `parse_gsd_session_no_session_line` | Tests parse_gsd_session |
| `parse_gsd_session_title_truncated_to_50_chars` | Tests parse_gsd_session — needs `use serde_json` |
| `parse_gsd_session_empty_file` | Tests parse_gsd_session |
| `discover_gsd_sessions_finds_by_workspace` | Cross-module: uses `encode_project_path` from config + `parse_gsd_session` — needs `use crate::config::encode_project_path; use serde_json` |

**util.rs (1 test):**
| Test | Notes |
|------|-------|
| `relative_time_formatting` | Tests now_secs + relative_time |

### Per-Module Test Block Import Patterns

Each module's test block will use `use super::*;` to access the module's own public items. Additional imports:

```rust
// config.rs tests — no extra imports needed (super::* covers everything)
#[cfg(test)]
mod tests {
    use super::*;

    // 8 tests...
}

// types.rs tests
#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json;

    use super::*;

    // 6 tests...
}

// discovery.rs tests
#[cfg(test)]
mod tests {
    use crate::config::encode_project_path;

    use serde_json;

    use super::*;

    // 18 tests...
}

// util.rs tests — no extra imports needed
#[cfg(test)]
mod tests {
    use super::*;

    // 1 test...
}
```

### main.rs After Migration

```rust
use amux::app;

fn main() -> anyhow::Result<()> {
    app::run()
}
```

Exactly 5 lines. No test module, no comment separator.

### Key Files

| File | Change |
|------|--------|
| `src/main.rs` | Remove lines 7–454 (comment + entire test module). Result: 5 lines. |
| `src/config.rs` | Append `#[cfg(test)] mod tests { ... }` with 8 tests at end of file. |
| `src/types.rs` | Append `#[cfg(test)] mod tests { ... }` with 6 tests at end of file. |
| `src/discovery.rs` | Append `#[cfg(test)] mod tests { ... }` with 18 tests at end of file. |
| `src/util.rs` | Append `#[cfg(test)] mod tests { ... }` with 1 test at end of file. |

### Natural Seams

The 4 target modules are fully independent — tests can be added to each in any order. The only sequential dependency is: add tests to all 4 modules first, then strip main.rs last (so `cargo test` never fails due to duplicate test names).

### First Proof

Run `cargo test` after full migration. Should show 33 tests passing from their new module locations (`config::tests::*`, `discovery::tests::*`, `types::tests::*`, `util::tests::*`).

### Verification

```bash
cargo test                          # 33 passed, 0 failed
cargo test -- --list                # Verify test paths are per-module
cargo clippy -- -D warnings         # exit 0
cargo fmt --all -- --check          # exit 0
wc -l src/main.rs                   # Should be 5 lines
grep -c '#\[test\]' src/main.rs     # Should be 0
grep -c '#\[test\]' src/config.rs src/types.rs src/discovery.rs src/util.rs  # Should total 33
```

## Constraints and Gotchas

- **serde_json import:** `config.rs` and `util.rs` don't currently import serde_json, but they don't need it for their tests (config tests don't use serde_json; the only util test doesn't either). Only `types.rs` and `discovery.rs` tests need `serde_json` — both files already have serde as a dependency in Cargo.toml.
- **No test logic changes:** Tests must be copied verbatim — only the import block at the top of each `mod tests` changes.
- **Test names are unique across modules:** All 33 test names are distinct, so no collision risk.
- **discover_gsd_sessions_finds_by_workspace** is the only cross-module test — it lives in discovery.rs but needs `crate::config::encode_project_path`. The `use crate::config::encode_project_path` import is straightforward since config is a sibling module.
- **Expanded field flattening:** In `config_roundtrip`, `assert!(!parsed.workspaces[1].expanded)` — expanded is not in the JSON (it's skipped during serialization). The test asserts the default `false`. This is testing the serde skip behavior of the `expanded` field. No change needed; just copy as-is.
