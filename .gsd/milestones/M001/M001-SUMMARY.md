---
id: M001
title: "GSD Agent Support"
status: complete
completed_at: 2026-06-02T13:23:02.733Z
key_decisions:
  - D001: GSD icon G + Magenta color, matching C/X single-letter pattern
  - D002: Resume via gsd -c (most recent session) since gsd CLI lacks --resume flag
  - D003: Title extraction prefers custom_message(gsd-run) then falls back to user message
  - D004: GSD sessions persist after PTY exit, matching Claude behavior
key_files:
  - src/types.rs
  - src/discovery.rs
  - src/util.rs
  - src/app.rs
  - src/main.rs
lessons_learned:
  - GSD directory encoding (/ → -) is lossy — hyphens in paths may decode incorrectly, accepted as known limitation
  - Task T02 was a no-op because T01 already covered its scope — task boundaries were too granular
  - GSD CLI lacks --resume <id> flag, limiting resume to gsd -c (most recent session)
  - Agent enum extension pattern: add variant + implement all helper methods in single match blocks for exhaustiveness
  - Agent picker guarded keybinding pattern: KeyCode + available_agents.contains() consistent across all three agents
---

# M001: GSD Agent Support

**Added GSD as a first-class agent in amux with session discovery from ~/.gsd/sessions/, agent picker quick-key G, and three-agent TUI integration — 33 tests passing.**

## What Happened

M001 added GSD (gsd CLI) as a third first-class agent in amux, completing the three-agent lineup alongside Claude Code and Codex.

**S01 (GSD Agent Core)** introduced the Agent::Gsd enum variant with all helper methods (cmd, label, icon, color, sessions_dir, build_new_cmd, build_resume_cmd), implemented GSD session discovery from ~/.gsd/sessions/ with JSONL v3 parsing and workspace grouping, and wired GSD detection into detect_agents() via which("gsd"). 30 unit tests validated the core. A minor deviation occurred: T02 was a no-op because T01's implementation already covered its scope.

**S02 (GSD UI Integration)** added the G quick-key to the agent picker popup with an available_agents guard, updated help text and error messages to include GSD alongside Claude Code and Codex, and confirmed all 30 tests pass. GSD sessions persist after PTY exit (matching Claude behavior, unlike Codex auto-cleanup). Resume uses gsd -c for the most recent session since gsd CLI lacks a --resume flag.

Cross-slice integration was clean: S01's enum and discovery surfaces were consumed directly by S02's UI layer. All 5 boundary contracts honored. 33 total tests pass with zero warnings. Runtime evidence confirms 14 GSD sessions discovered and correctly grouped by workspace.

## Success Criteria Results

- [x] cargo test passes with GSD-specific unit tests — 33 tests pass (0 failed)
- [x] discover_gsd_sessions() returns sessions grouped by workspace from ~/.gsd/sessions/ — verified by 6 GSD-specific unit tests + cargo run showing 14 sessions discovered
- [x] Agent::Gsd variant exposes icon G, color Magenta, label GSD, cmd gsd — verified by agent_traits test + exhaustive match compilation
- [x] Agent picker G keybinding wired with available_agents.contains guard — verified by compilation + grep
- [x] build_resume_cmd() produces correct CommandBuilder with workspace CWD — verified by unit test
- [x] GSD sessions not auto-cleaned after PTY exit — verified by gsd_sessions_persist_after_pty_exit test
- [x] detect_agents() omits Agent::Gsd when gsd not installed — verified by which() logic

## Definition of Done Results

- [x] All slices complete: S01 ✅, S02 ✅
- [x] All summaries exist: S01-SUMMARY.md, S02-SUMMARY.md, plus all task summaries
- [x] Integration works: 33 tests pass, cargo run discovers 14 GSD sessions grouped by workspace
- [x] Validation passed: M001-VALIDATION.md verdict pass with all success criteria met

## Requirement Outcomes

- R001: active → validated — detect_agents() checks which("gsd") and pushes Agent::Gsd if found. Verified by cargo test (33 tests pass).
- R002: active → validated — discover_gsd_sessions() scans ~/.gsd/sessions/ subdirectories, parses JSONL v3, matches to workspaces by decoded dir names. Verified by 6 GSD-specific unit tests.
- R005: active → validated — Agent::Gsd variant exists with cmd="gsd", label="GSD", icon="G", color=Magenta, sessions_dir(), build_new_cmd(), build_resume_cmd(). All match arms exhaustive, verified by cargo test.

## Deviations

S01 T02 was a no-op — T01 had already implemented all T02 deliverables. No code changes or negative impact; accepted as a planning granularity issue.

## Follow-ups

None — three-agent architecture is complete. Future work could include: (1) per-session GSD resume if gsd CLI adds --resume flag, (2) GSD session metadata display (status, duration), (3) CI integration for automated test runs.
