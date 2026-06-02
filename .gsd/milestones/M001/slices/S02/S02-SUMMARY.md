---
id: S02
parent: M001
milestone: M001
provides:
  - GSD quick-key G in agent picker
  - Agent-agnostic help text and error messages
  - Complete three-agent lineup in TUI
requires:
  - slice: S01
    provides: Agent::Gsd variant with cmd/label/icon/color/sessions_dir/build_new_cmd/build_resume_cmd, detect_agents() returning Agent::Gsd when gsd CLI is found, discover_gsd_sessions() for workspace session enumeration
affects:
  []
key_files:
  - src/app.rs
key_decisions: []
patterns_established:
  - Agent picker uses guarded keybinding pattern: KeyCode match + available_agents.contains() check, consistent across all three agents
observability_surfaces:
  - none
drill_down_paths:
  - .gsd/milestones/M001/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M001/slices/S02/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-02T11:24:25.018Z
blocker_discovered: false
---

# S02: GSD UI Integration: Agent Picker, PTY Lifecycle, and Resume

**Added GSD quick-key G to agent picker popup, updated help text and error messages to include GSD alongside Claude Code and Codex, confirmed all 30 tests pass with zero warnings.**

## What Happened

S02 wired the GSD agent variant (from S01) into the TUI's agent picker and status messaging. Two tasks were completed:

**T01** added the GSD quick-key binding in `handle_agent_key()` (app.rs lines 478-484): pressing 'g' or 'G' when `Agent::Gsd` is in `available_agents` selects it. The help text line was updated to `" C:Claude  X:Codex  G:GSD  j/k:navigate  Enter:confirm  Esc:cancel"`. The "no agent found" error was updated to include GSD. The kill/status message was already agent-agnostic ("Session terminated"). Build completed with 0 errors, 0 warnings.

**T02** ran the full test suite (30 passed, 0 failed) and confirmed clean compilation with zero warnings after the GSD keybinding changes.

Both tasks completed without deviations or known issues. The slice is the final piece of the M001 milestone, completing the three-agent (Claude Code, Codex, GSD) lineup in the amux TUI.

## Verification

**Build:** `cargo build` — Finished dev profile, 0 errors, 0 warnings (103ms).
**Tests:** `cargo test` — 30 passed, 0 failed, 0 ignored (0.01s).
**Grep verification:** Agent::Gsd references present at app.rs lines 478-484 (keybinding), 1363 (help text "G:GSD"), and 1494 (error message including GSD). Kill message at line 235 is agent-agnostic. Remaining "Claude Code" references at line 1195 (named session prompt) and 1494 (combined agent error) are appropriate.

## Requirements Advanced

- R001 — detect_agents() from S01 now surfaces GSD in the agent picker via the G quick-key, completing the discovery-to-UI chain
- R005 — Agent::Gsd icon/label/color are now consumed by the agent picker help text and keybinding display

## Requirements Validated

- R005 — Agent::Gsd icon='G' and color=Magenta are referenced in the picker help text ('G:GSD') and keybinding guard, verified by grep + cargo build + cargo test (30 pass)

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None.

## Known Limitations

GSD PTY resume uses gsd -c (most recent session) rather than targeting a specific session ID, matching the gsd CLI's current capability. Named session prompt (line 1195) remains Claude Code-specific since only Claude Code supports named sessions.

## Follow-ups

None — this is the final slice of M001.

## Files Created/Modified

- `src/app.rs` — Added GSD quick-key G binding in handle_agent_key(), updated help text to include G:GSD, updated no-agent-found error to include GSD
