---
id: S01
parent: M001
milestone: M001
provides:
  - Agent::Gsd enum variant with complete helper methods
  - discover_gsd_sessions() for scanning ~/.gsd/sessions/
  - parse_gsd_session() for JSONL v3 parsing with title extraction
  - GSD detection in detect_agents() via which("gsd")
requires:
  []
affects:
  - S02
key_files:
  - src/types.rs
  - src/discovery.rs
  - src/util.rs
  - src/main.rs
key_decisions:
  - GSD directory encoding uses simple replace(/, -) matching GSD's own scheme, accepting lossy behavior for paths with hyphens
  - GSD resume command uses 'gsd sessions' (interactive) rather than a direct --resume flag, since gsd CLI lacks one
patterns_established:
  - Agent enum extension pattern: add variant + implement all helper methods in single match blocks
  - Session discovery pattern: scan agent-specific sessions directory, parse agent-specific format, match to workspaces by decoded dir name
observability_surfaces:
  - none
drill_down_paths:
  - .gsd/milestones/M001/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M001/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M001/slices/S01/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-02T11:12:10.187Z
blocker_discovered: false
---

# S01: GSD Agent Core: Enum, Discovery, and Session Parsing

**Added Agent::Gsd enum variant with all helper methods (cmd/label/icon/color/sessions_dir/build_new_cmd/build_resume_cmd), implemented GSD session discovery from ~/.gsd/sessions/ with JSONL v3 parsing and workspace grouping, wired GSD detection into detect_agents(), and verified with 30 passing unit tests.**

## What Happened

T01 added the Agent::Gsd enum variant to types.rs with all required helper methods: cmd() returns "gsd", label() returns "GSD", icon() returns "G", color() returns Color::Magenta, sessions_dir() returns ~/.gsd/sessions/ when the directory exists, build_new_cmd() creates a CommandBuilder for `gsd` with workspace CWD, and build_resume_cmd() creates a CommandBuilder for `gsd sessions` with workspace CWD. T01 also implemented discover_gsd_sessions() and parse_gsd_session() in discovery.rs.

T02 verified that all GSD session discovery and JSONL parsing functionality was already complete from T01's implementation. All 25 tests passed including the 6 GSD-specific tests.

T03 wired GSD detection into detect_agents() in util.rs by checking `which("gsd")` and pushing Agent::Gsd when found. It also added comprehensive unit tests covering edge cases: empty file parsing, directory name encoding/decoding roundtrip, simple and root decode, and end-to-end discovery with workspace matching. All 30 tests pass with 0 failures.

A notable finding: GSD directory name encoding (replacing '/' with '-') is lossy — hyphens in original workspace paths become ambiguous when decoding. This is a known limitation of the encoding scheme, not a bug.

## Verification

cargo test runs 30 tests with 0 failures. GSD-specific tests cover: valid JSONL parsing with gsd-run title extraction, fallback to user message, gsd-run priority over user message, handling of missing session line, title truncation to 50 chars, directory name encoding/decoding, empty file handling, root path decode, and end-to-end discovery with workspace matching. All match arms for Agent enum are exhaustive across types.rs and discovery.rs. GSD detection correctly wired in detect_agents().

## Requirements Advanced

- R001 — detect_agents() now checks which("gsd") and pushes Agent::Gsd when found
- R002 — discover_gsd_sessions() scans ~/.gsd/sessions/, parses JSONL v3, groups by workspace
- R005 — Agent::Gsd variant added with all helper methods (cmd, label, icon, color, sessions_dir, build_new_cmd, build_resume_cmd)
- R007 — parse_gsd_session extracts title preferring gsd-run custom_message, falling back to user message, truncated to 50 chars

## Requirements Validated

- R001 — detect_agents() in util.rs checks which("gsd") — 30 cargo tests pass
- R002 — discover_gsd_sessions() with JSONL parsing — 6 GSD-specific unit tests pass
- R005 — Agent::Gsd variant with all methods — exhaustive match arms verified by compilation + tests
- R007 — Title extraction from JSONL — tests for gsd-run priority, user message fallback, and truncation pass
- R009 — 30 unit tests pass covering enum properties, session parsing, directory encoding, and edge cases

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

T02 required no code changes — T01 had already implemented all T02 deliverables. T03's encode/decode roundtrip test uses a hyphen-free path because the GSD directory encoding is lossy (hyphens in original paths become ambiguous). This is documented as a known limitation.

## Known Limitations

GSD directory name encoding (/ → -) is lossy: workspace paths containing hyphens may decode incorrectly. The codebase uses a simple replace scheme matching GSD's behavior, but this means /home/user/my-project and /home/user/my+project would collide. This is a known limitation of GSD's own encoding, not introduced by amux.

## Follow-ups

None — S02 (UI integration) is the natural next step and depends on S01's enum and discovery surfaces.

## Files Created/Modified

None.
