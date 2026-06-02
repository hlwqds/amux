# M001: GSD Agent Support

**Gathered:** 2026-06-02
**Status:** Ready for planning

## Project Description

Add GSD (gsd CLI) as a first-class agent in amux, alongside Claude Code and Codex. This includes agent detection, session discovery from `~/.gsd/sessions/`, new session spawn, session resume, and full integration with the existing agent framework.

## Why This Milestone

amux is a multi-agent TUI aggregator. GSD is a third agent CLI the user actively works with. Currently only Claude Code and Codex are supported — adding GSD completes the agent lineup and removes the need to manage GSD sessions outside amux.

## User-Facing Capabilities

- GSD sessions are discovered from ~/.gsd/sessions/ and grouped by workspace
- GSD agent is included in detect_agents() output when gsd CLI is installed
- Agent picker G keybinding is wired for GSD when available
- GSD session resume command builds correct CommandBuilder with workspace CWD
- GSD sessions are not auto-cleaned after PTY exit (unlike Codex)

### Entry point / environment

- Entry point: `amux` CLI
- Environment: local terminal (Linux x86_64)
- Live dependencies involved: `gsd` CLI, `~/.gsd/sessions/` filesystem

## Completion Class

- Contract complete means: GSD agent enum, discovery, spawn, resume all implemented with passing unit tests
- Integration complete means: All code paths compile, agent picker keybinding wired, spawn/resume CommandBuilders tested, session discovery verified by cargo run output
- Operational complete means: All lifecycle code paths tested — discover, spawn, resume, persistence after exit

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- discover_gsd_sessions() returns sessions from ~/.gsd/sessions/ grouped by workspace (unit tests + cargo run)
- Agent::Gsd variant has all required methods and is returned by detect_agents() when gsd is installed
- Agent picker G keybinding is compiled and guarded by available_agents check
- GSD sessions persist after PTY exit — poll_states() retain filter only removes Agent::Codex
- When gsd is not installed, detect_agents() omits Agent::Gsd

## Architectural Decisions

### GSD Agent Icon and Color

**Decision:** Use "G" as icon and Magenta as color for the GSD agent variant.

**Rationale:** Claude uses "C" (Cyan) and Codex uses "X" (Green). "G" continues the single-letter pattern. Magenta is visually distinct from the existing cyan/green palette.

**Alternatives Considered:**
- "D" (for Done/Ship) — less intuitive than matching the name
- Yellow — conflicts with running-state indicator color

### GSD Session Resume Strategy

**Decision:** Resume via stdin pipe to `gsd sessions` interactive picker.

**Rationale:** GSD has no `--resume <id>` CLI flag. The only resume mechanisms are `gsd -c` (most recent only) and `gsd sessions` (interactive numbered list). Piping the session number via PTY stdin is the only way to resume a specific session.

**Alternatives Considered:**
- `gsd -c` only — limits resume to most recent session, not parity with Claude/Codex
- Wait for GSD to add resume flag — blocks this milestone indefinitely

### GSD Session Title Extraction

**Decision:** Extract from `custom_message` with `customType: "gsd-run"` first, fall back to `message` with `role: "user"`.

**Rationale:** GSD wraps user prompts in `custom_message` records with `gsd-run` type during auto-mode. Direct user messages use standard `message` records. Prefer gsd-run as it's the richer source.

**Alternatives Considered:**
- Only use `message` records — misses auto-mode session titles
- Only use `custom_message` — misses interactive session titles

## Error Handling Strategy

Follow existing agent patterns — no special error handling for GSD:

- CLI not installed → not detected, not shown
- `~/.gsd/sessions/` missing → `sessions_dir()` returns None, skip discovery
- JSONL parse failure → skip individual file (return None)
- Resume target mismatch → user sees `gsd sessions` output in PTY, can manually select correct session

## Risks and Unknowns

- **GSD session resume ordering** — stdin pipe sends a number to `gsd sessions`, but if GSD's listing order doesn't match our scan order, the wrong session resumes. Mitigation: PTY shows the actual selection so user can see what happened.
- **GSD JSONL format stability** — format is external contract. v3 is current. If GSD changes format, discovery breaks silently (sessions don't appear, no crash).

## Existing Codebase / Prior Art

- `src/types.rs` — `Agent` enum with Claude/Codex variants, helper methods, `build_new_cmd`/`build_resume_cmd`
- `src/discovery.rs` — `discover_claude_sessions()`, `discover_codex_sessions()`, `parse_codex_session()`, `clean_user_message()`, `extract_text_from_content()`
- `src/util.rs` — `detect_agents()`, `which()`
- `src/pty.rs` — `PtyHandle::spawn()` — agent-agnostic, accepts CommandBuilder
- `src/app.rs` — `poll_states()` has Codex-specific cleanup logic; agent picker with C/X quick keys

## Relevant Requirements

- R001 — GSD agent detection
- R002 — GSD session discovery
- R003 — GSD new session spawn
- R004 — GSD session resume
- R005 — Agent enum extension
- R006 — Agent picker GSD support
- R007 — GSD session title extraction
- R008 — Codex-style cleanup exemption
- R009 — Unit tests

## Scope

### In Scope

- Agent enum GSD variant with all helper methods
- GSD session discovery from `~/.gsd/sessions/`
- New session spawn via `gsd` command
- Session resume via `gsd sessions` stdin pipe
- Agent picker GSD option with quick-key G
- PTY lifecycle: don't auto-remove GSD sessions on exit
- Unit tests for JSONL parsing and enum properties

### Out of Scope / Non-Goals

- None for this milestone

## Technical Constraints

- Linux x86_64 target
- Rust edition 2024
- No new crate dependencies
- Must not break existing Claude/Codex functionality

## Integration Points

- `gsd` CLI — spawn and resume commands
- `~/.gsd/sessions/` filesystem — session JSONL files
- `~/.gsd/` directory structure — session directory naming convention (path-encoded)

## Testing Requirements

- Unit tests: GSD session JSONL parsing, Agent::Gsd properties, dir name encoding
- Manual integration: verify cargo test passes and cargo run discovers GSD sessions
- Regression: verify cargo test passes, no impact on Claude/Codex

## Acceptance Criteria

- S01: cargo test passes with new GSD tests; GSD sessions discovered from ~/.gsd/sessions/; new GSD session CommandBuilder works
- S02: Agent picker G keybinding wired with available_agents guard; resume CommandBuilder works; GSD sessions persist after PTY exit

## Open Questions

- GSD session resume ordering risk — need manual verification that stdin pipe approach works correctly
