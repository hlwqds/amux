# S01: Sort modes and agent grouping — UAT

**Milestone:** M004
**Written:** 2026-06-02T17:10:33.203Z

# S01: Sort modes and agent grouping — UAT

**Milestone:** M004
**Written:** 2026-06-02

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: All functionality is verified through unit tests (sort logic, keybinding wiring, UI rendering via match-arm coverage). The TUI is a terminal application with no external dependencies.

## Preconditions

- Project builds: `cargo build` succeeds
- Test suite passes: `cargo test` shows 61 passed, 0 failed

## Smoke Test

```bash
cargo test --lib -- sort 2>&1 | grep "test result"
```
Expected: all sort-related tests pass.

## Test Cases

### 1. Sort mode cycling

1. Run `cargo test --lib -- sort_mode_cycles_through_all_variants`
2. **Expected:** Test passes — `next()` visits all 5 variants and wraps back to TimeDesc

### 2. Default sort mode

1. Run `cargo test --lib -- sort_mode_default_is_time_desc`
2. **Expected:** Test passes — Default is TimeDesc

### 3. Time Desc sort (newest first)

1. Run `cargo test --lib -- sort_time_desc_newest_first`
2. **Expected:** Test passes — sessions ordered by last_active descending

### 4. Time Asc sort (oldest first)

1. Run `cargo test --lib -- sort_time_asc_oldest_first`
2. **Expected:** Test passes — sessions ordered by last_active ascending

### 5. Name A→Z sort

1. Run `cargo test --lib -- sort_name_asc_alphabetical`
2. **Expected:** Test passes — case-insensitive alphabetical by title

### 6. Name Z→A sort

1. Run `cargo test --lib -- sort_name_desc_reverse_alphabetical`
2. **Expected:** Test passes — case-insensitive reverse alphabetical

### 7. Agent Group sort

1. Run `cargo test --lib -- sort_agent_group`
2. **Expected:** Both agent_group tests pass — sessions grouped by agent type with headers, empty groups omitted

### 8. Sort + filter integration

1. Run `cargo test --lib -- sort_with_active_filter`
2. **Expected:** Test passes — sort reorders filtered results; filter applied first

### 9. AgentHeader inertness

1. Run `cargo test --lib -- agent_header_is_inert`
2. **Expected:** Both inertness tests pass — activate and delete on AgentHeader return None

### 10. Selection clamping

1. Run `cargo test --lib -- sort_preserves_selection_clamping`
2. **Expected:** Test passes — selection clamped to valid range after sort change

## Edge Cases

### Empty session list
- Sort on empty sessions: no panic, tree is empty

### Single session
- All sort modes produce identical single-element tree

### Mixed case titles
- Name sorts are case-insensitive: "alpha", "Bravo", "charlie" ordered correctly regardless of case

## Failure Signals

- Any test failure in `cargo test`
- Clippy warnings with `-D warnings`
- Fmt drift detected by `cargo fmt --check`

## Not Proven By This UAT

- Live TUI rendering of AgentHeader visual style (tested structurally via match-arm coverage)
- Actual keypress `s` in running application (wiring verified by code review + compilation)
- Concurrent sort cycling during ongoing search input

## Notes for Tester

- This is a terminal TUI application. Full interactive testing requires running `cargo run` with configured agent workspaces.
- The 12 new tests provide comprehensive coverage of sort logic, making interactive testing a complementary (not primary) verification.
