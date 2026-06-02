# Requirements

This file is the explicit capability and coverage contract for the project.

Use it to track what is actively in scope, what has been validated by completed work, what is intentionally deferred, and what is explicitly out of scope.

Guidelines:
- Keep requirements capability-oriented, not a giant feature wishlist.
- Requirements should be atomic, testable, and stated in plain language.
- Every **Active** requirement should be mapped to a slice, deferred, blocked with reason, or moved out of scope.
- Each requirement should have one accountable primary owner and may have supporting slices.
- Research may suggest requirements, but research does not silently make them binding.
- Validation means the requirement was actually proven by completed work and verification, not just discussed.

## Active

### R001 — GSD agent detection
- Class: core-capability
- Status: active
- Description: amux detects whether the `gsd` CLI is installed (via `which("gsd")`) and includes it in the available agents list if present
- Why it matters: Without detection, GSD never appears as an option
- Source: inferred
- Primary owning slice: M001/S01
- Supporting slices: none
- Validation: mapped
- Notes: Follows existing `detect_agents()` pattern

### R002 — GSD session discovery
- Class: core-capability
- Status: active
- Description: amux scans `~/.gsd/sessions/` directories, parses session JSONL files, and surfaces GSD sessions in the sidebar grouped by workspace
- Why it matters: Users need to see and interact with existing GSD sessions
- Source: inferred
- Primary owning slice: M001/S01
- Supporting slices: none
- Validation: mapped
- Notes: Session dir names encode workspace path (/ → -). JSONL v3 format: first line has {type:"session",id,cwd}. Title from custom_message gsd-run or user message.

### R003 — GSD new session spawn
- Class: primary-user-loop
- Status: active
- Description: User can spawn a new GSD session in any workspace via agent picker, running `gsd` in a PTY
- Why it matters: Core user workflow — starting new agent sessions
- Source: user
- Primary owning slice: M001/S01
- Supporting slices: none
- Validation: mapped
- Notes: Uses existing PTY spawn infrastructure

### R004 — GSD session resume
- Class: primary-user-loop
- Status: active
- Description: User can resume an existing GSD session by selecting it in the sidebar, piping session index to `gsd sessions` via PTY stdin
- Why it matters: Core user workflow — resuming past sessions
- Source: inferred
- Primary owning slice: M001/S01
- Supporting slices: M001/S02
- Validation: mapped
- Notes: GSD lacks --resume <id> flag. Resume via stdin pipe to `gsd sessions` interactive picker. Risk: ordering mismatch.

### R005 — Agent enum extension
- Class: core-capability
- Status: active
- Description: `Agent` enum in types.rs gains a `Gsd` variant with cmd/label/icon/color/sessions_dir/build_new_cmd/build_resume_cmd
- Why it matters: Foundation for all GSD support — every other feature depends on this
- Source: inferred
- Primary owning slice: M001/S01
- Supporting slices: none
- Validation: mapped
- Notes: icon="G", color=Magenta, cmd="gsd"

### R006 — Agent picker GSD support
- Class: core-capability
- Status: active
- Description: GSD appears in the agent picker popup with quick-key G, matching existing C/X pattern
- Why it matters: Users need a way to select GSD when creating sessions
- Source: inferred
- Primary owning slice: M001/S02
- Supporting slices: none
- Validation: mapped
- Notes: Also handles single-agent auto-select when only GSD is installed

### R007 — GSD session title extraction
- Class: core-capability
- Status: active
- Description: Extract first user message from GSD session JSONL as session title — prefer custom_message with customType "gsd-run", fall back to role=user message
- Why it matters: Sessions need readable titles in sidebar
- Source: inferred
- Primary owning slice: M001/S01
- Supporting slices: none
- Validation: mapped
- Notes: Truncate to 50 chars like Claude/Codex

### R008 — Codex-style cleanup exemption for GSD
- Class: continuity
- Status: active
- Description: GSD sessions are NOT removed when PTY process exits (unlike Codex), so users can resume them later
- Why it matters: Without this, completed GSD sessions vanish from sidebar
- Source: inferred
- Primary owning slice: M001/S02
- Supporting slices: none
- Validation: mapped
- Notes: Modify poll_states() to check agent type before removing

### R009 — Unit tests for GSD session parsing
- Class: quality-attribute
- Status: active
- Description: Unit tests covering GSD session JSONL parsing, dir name encoding, and Agent::Gsd enum properties
- Why it matters: Prevents regression and validates parsing against format edge cases
- Source: inferred
- Primary owning slice: M001/S01
- Supporting slices: none
- Validation: mapped
- Notes: Follow pattern of existing parse_codex_session tests

## Validated

(none yet)

## Deferred

(none)

## Out of Scope

(none)

## Traceability

| ID | Class | Status | Primary owner | Supporting | Proof |
|---|---|---|---|---|---|
| R001 | core-capability | active | M001/S01 | none | mapped |
| R002 | core-capability | active | M001/S01 | none | mapped |
| R003 | primary-user-loop | active | M001/S01 | none | mapped |
| R004 | primary-user-loop | active | M001/S01 | M001/S02 | mapped |
| R005 | core-capability | active | M001/S01 | none | mapped |
| R006 | core-capability | active | M001/S02 | none | mapped |
| R007 | core-capability | active | M001/S01 | none | mapped |
| R008 | continuity | active | M001/S02 | none | mapped |
| R009 | quality-attribute | active | M001/S01 | none | mapped |

## Coverage Summary

- Active requirements: 9
- Mapped to slices: 9
- Validated: 0
- Unmapped active requirements: 0
