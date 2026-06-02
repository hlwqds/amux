# S01: GSD Agent Core: Enum, Discovery, and Session Parsing — UAT

**Milestone:** M001
**Written:** 2026-06-02T11:12:10.188Z

# S01: GSD Agent Core: Enum, Discovery, and Session Parsing — UAT

**Milestone:** M001
**Written:** 2026-06-02

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: This slice is purely backend — enum variant, parsing logic, and discovery functions. All verification is contract-level (unit tests) with no TUI or runtime dependency.

## Preconditions

- Rust toolchain installed (cargo available)
- Source code at `.gsd/worktrees/M001`

## Smoke Test

```bash
cargo test
```
**Expected:** 30 passed, 0 failed.

## Test Cases

### 1. Agent::Gsd enum properties

1. Run `cargo test parse_gsd_session`
2. **Expected:** 6 GSD-specific tests pass covering JSONL parsing, title extraction (gsd-run priority, user message fallback, truncation), and directory encoding.

### 2. Directory name encoding/decoding

1. Run `cargo test gsd_directory_name`
2. **Expected:** Tests for encoding, decoding, root path, and roundtrip all pass.

### 3. GSD detection in detect_agents()

1. Check `src/util.rs` contains `which("gsd")` check
2. Run `cargo test`
3. **Expected:** All 30 tests pass, including tests that verify GSD is included in agent list when `which("gsd")` succeeds.

### 4. Session discovery with workspace matching

1. Run `cargo test discover`
2. **Expected:** End-to-end discovery test passes, matching sessions to workspaces by decoded directory name.

## Edge Cases

- **Empty JSONL file:** parse_gsd_session returns None (no panic)
- **Missing session line:** parse_gsd_session returns None gracefully
- **Title > 50 chars:** Truncated to 50 characters
- **No ~/.gsd/sessions/ directory:** discover_gsd_sessions returns no sessions (no error)
- **Lossy encoding:** Paths with hyphens may decode incorrectly — known limitation

## Gated by

- All 30 cargo tests pass ✅
