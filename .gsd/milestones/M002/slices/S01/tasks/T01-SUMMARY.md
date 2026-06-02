---
id: T01
parent: S01
milestone: M002
key_files:
  - src/lib.rs
  - src/main.rs
key_decisions:
  - Used package name 'amux' (from Cargo.toml) as the lib crate name for imports in main.rs and tests
duration: 
verification_result: passed
completed_at: 2026-06-02T14:35:08.282Z
blocker_discovered: false
---

# T01: Created src/lib.rs as library root and rewired main.rs to use amux crate imports

**Created src/lib.rs as library root and rewired main.rs to use amux crate imports**

## What Happened

Created src/lib.rs with `pub mod` declarations for all 6 modules (app, config, discovery, pty, types, util). Rewired src/main.rs: removed all 6 `mod` declarations, added `use amux::app;` at the top, and changed test imports from `super::*` to `amux::*` for config, discovery, types, and util. Cargo build succeeds and all 33 tests pass.

## Verification

cargo build exits 0, cargo test shows 33 passed / 0 failed. src/lib.rs exists as a valid library root with all module declarations.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo build` | 0 | ✅ pass | 1067ms |
| 2 | `cargo test` | 0 | ✅ pass | 516ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/lib.rs`
- `src/main.rs`
