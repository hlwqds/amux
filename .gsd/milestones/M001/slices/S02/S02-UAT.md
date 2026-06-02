# S02: GSD UI Integration: Agent Picker, PTY Lifecycle, and Resume — UAT

**Milestone:** M001
**Written:** 2026-06-02T11:24:25.018Z

# S02: GSD UI Integration: Agent Picker, PTY Lifecycle, and Resume — UAT

**Milestone:** M001
**Written:** 2026-06-02

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: This slice is a wiring/integration slice — the deliverables are source code changes (keybinding, strings) verified by compilation and test suite. No live runtime testing needed beyond build + test.

## Preconditions

- Rust toolchain installed
- Repository checked out at the worktree
- S01 completed (Agent::Gsd variant, detect_agents(), discover_gsd_sessions() all available)

## Smoke Test

`cargo build && cargo test` — both pass with zero errors/failures/warnings.

## Test Cases

### 1. GSD quick-key binding compiles and is reachable

1. `grep -n "KeyCode::Char('g').*Agent::Gsd" src/app.rs`
2. **Expected:** Match at line ~478 showing 'g'/'G' key bound to `Agent::Gsd` selection when agent is available.

### 2. Help text includes G:GSD

1. `grep -n "G:GSD" src/app.rs`
2. **Expected:** Match at line ~1363 in the agent picker help string.

### 3. No-agent-found error includes GSD

1. `grep -n "Install Claude Code, Codex, or GSD" src/app.rs`
2. **Expected:** Match at line ~1494 in the bail!() macro.

### 4. Kill message is agent-agnostic

1. `grep -n "Session terminated" src/app.rs`
2. **Expected:** Match at line ~235 showing generic "Session terminated" text without agent-specific name.

### 5. Full test suite passes

1. `cargo test`
2. **Expected:** 30 passed, 0 failed.

### 6. Clean build with no warnings

1. `cargo build`
2. **Expected:** Finished with 0 warnings.

## Edge Cases

### GSD key only active when gsd CLI is installed

1. The keybinding guards with `if self.available_agents.contains(&Agent::Gsd)`, so pressing G when gsd is not installed falls through to the default key handler.
2. **Expected:** No crash, no GSD option appears.

## Failure Signals

- cargo build produces warnings or errors
- cargo test has any failures
- "G:GSD" missing from help text
- Agent::Gsd not referenced in keybinding match arm

## Not Proven By This UAT

- Live TUI rendering of GSD in the agent picker popup (would require interactive terminal)
- PTY spawning and session resume behavior (runtime integration, not compilation)
- GSD sessions appearing in sidebar (depends on gsd CLI being installed and having session data)

## Notes for Tester

- This is the final slice of M001. Upon completion, the three-agent lineup (Claude Code, Codex, GSD) is feature-complete.
- Live runtime testing of the agent picker would require `gsd` CLI to be installed and session data present in `~/.gsd/sessions/`.

