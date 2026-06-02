# S02: GSD UI Integration: Agent Picker, PTY Lifecycle, and Resume

**Goal:** Wire GSD into the TUI agent picker (quick-key G), update help text and status messages to be agent-agnostic, and confirm GSD sessions persist after PTY exit via existing cleanup logic.
**Demo:** Agent picker popup shows GSD with quick-key G and Magenta color. GSD sessions appear in sidebar. Clicking a GSD session resumes via gsd -c in PTY. GSD sessions persist after PTY exit.

## Must-Haves

- Agent picker responds to 'G' key when GSD is available, selecting and confirming GSD agent
- Help text in agent picker popup includes "G:GSD"
- Kill message uses agent-agnostic text instead of "Claude Code"
- No-agent-found error message includes GSD
- cargo test passes with 0 failures
- cargo build succeeds with 0 warnings

## Proof Level

- This slice proves: integration — exercises the TUI keybinding wiring and confirms no regressions in compilation and test suite

## Integration Closure

- Upstream surfaces consumed: Agent::Gsd variant and methods from S01 (types.rs), detect_agents() from S01 (util.rs), discover_gsd_sessions() from S01 (discovery.rs)
- New wiring introduced in this slice: agent picker quick-key 'G' binding in handle_agent_key(), updated help/status strings referencing GSD
- What remains before the milestone is truly usable end-to-end: nothing — this is the final slice, completing the three-agent lineup

## Verification

- Run the task and slice verification checks for this slice.

## Tasks

- [x] **T01: Add GSD quick-key, help text, and agent-agnostic strings to app.rs** `est:20m`
  Why: The TUI agent picker needs a GSD quick-key binding and all hardcoded "Claude Code" strings need to be agent-agnostic so the three-agent lineup is complete.
  - Files: `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/app.rs`
  - Verify: cargo build

- [x] **T02: Verify all tests pass and GSD keybinding compiles correctly** `est:10m`
  Why: Must confirm that the S02 changes don't break any of the 30 existing tests and that the new GSD keybinding match arm compiles correctly with exhaustive pattern coverage.
  - Files: `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/app.rs`
  - Verify: cargo test

## Files Likely Touched

- /home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/app.rs
