# S01: GSD Agent Core: Enum, Discovery, and Session Parsing

**Goal:** Add GSD agent as a first-class variant in the Agent enum with all helper methods, implement GSD session discovery from ~/.gsd/sessions/ with JSONL v3 parsing and workspace grouping, wire GSD detection into detect_agents(), and write comprehensive unit tests covering enum properties, session parsing, and directory name encoding.
**Demo:** cargo test passes with GSD JSONL parsing tests, Agent::Gsd variant has correct icon/label/color/cmd/sessions_dir, discover_gsd_sessions() finds sessions from ~/.gsd/sessions/ with correct workspace grouping and title extraction

## Must-Haves

- Agent::Gsd variant exists with cmd="gsd", icon="G", color=Magenta, label="GSD"
- sessions_dir() returns Some(~/.gsd/sessions/) when directory exists
- build_new_cmd() produces correct CommandBuilder for `gsd` with workspace CWD and env setup
- build_resume_cmd() produces correct CommandBuilder for `gsd sessions` with workspace CWD
- discover_gsd_sessions() finds sessions from ~/.gsd/sessions/ subdirectories
- GSD sessions are correctly matched to workspaces by decoding dir names (/ → -)
- Title extraction prefers custom_message(gsd-run) then falls back to user message
- detect_agents() includes GSD when `gsd` CLI is found
- All new unit tests pass via cargo test

## Proof Level

- This slice proves: contract — enum properties, JSONL parsing, and dir-name encoding verified by unit tests without needing a running TUI or gsd CLI

## Integration Closure

- Upstream surfaces consumed: Agent enum (types.rs), discovery pattern (discovery.rs), detect_agents (util.rs)
- New wiring introduced in this slice: Agent::Gsd variant in types.rs, discover_gsd_sessions() in discovery.rs, gsd detection in detect_agents()
- What remains before the milestone is truly usable end-to-end: S02 wires Agent::Gsd into the agent picker UI, sidebar rendering, poll_states() cleanup exemption, and resume stdin pipe logic

## Verification

- Signals added/changed: Agent::Gsd color/icon in sidebar rendering; sessions discovered from ~/.gsd/sessions/
- Inspection surfaces: cargo test output shows GSD-specific test results; discover_sessions() returns GSD sessions alongside Claude/Codex
- Failure visibility: JSONL parse failure silently skips individual sessions (returns None); missing ~/.gsd/sessions/ directory returns no sessions (no error)

## Tasks

- [x] **T01: Add Agent::Gsd enum variant with all helper methods** `est:30m`
  Why: Foundation for all GSD support — every other feature depends on the Agent enum having a Gsd variant.
  - Files: `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/types.rs`, `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/discovery.rs`
  - Verify: cargo test

- [x] **T02: Implement GSD session discovery and JSONL parsing** `est:45m`
  Why: Users need to see existing GSD sessions in the sidebar. This requires scanning ~/.gsd/sessions/ directories, parsing GSD's JSONL v3 format, matching sessions to workspaces by decoding directory names, and extracting session titles.
  - Files: `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/discovery.rs`, `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/types.rs`
  - Verify: cargo test

- [x] **T03: Wire GSD detection into detect_agents() and add comprehensive unit tests** `est:30m`
  Why: GSD must be detected via `which("gsd")` to appear in the available agents list, and all new code needs test coverage (R009).
  - Files: `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/util.rs`, `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/main.rs`, `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/discovery.rs`
  - Verify: cargo test

## Files Likely Touched

- /home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/types.rs
- /home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/discovery.rs
- /home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/util.rs
- /home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/main.rs
