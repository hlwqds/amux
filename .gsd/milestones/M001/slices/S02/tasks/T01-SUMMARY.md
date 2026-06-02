---
id: T01
parent: S02
milestone: M001
key_files:
  - src/app.rs
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-02T11:22:09.635Z
blocker_discovered: false
---

# T01: Added GSD quick-key G to agent picker, updated help/status strings to be agent-agnostic

**Added GSD quick-key G to agent picker, updated help/status strings to be agent-agnostic**

## What Happened

Applied all five targeted edits to src/app.rs:

1. **Agent picker quick-key 'G'**: Added a new match arm in `handle_agent_key()` after the X/Codex block, following the exact same pattern (KeyCode::Char('g')|('G'), checks available_agents for Agent::Gsd, selects and confirms).

2. **Help text in agent picker**: Changed `" C:Claude  X:Codex  j/k:navigate..."` to include `"G:GSD"`.

3. **Kill status message**: Changed `"Claude Code terminated. Sessions refreshed."` to `"Session terminated. Sessions refreshed."`.

4. **Help panel kill line**: Changed `"Ctrl+Q       Kill current Claude Code session"` to `"Ctrl+Q       Kill current session"`.

5. **No-agent error**: Changed the bail message from `"Install Claude Code or Codex."` to `"Install Claude Code, Codex, or GSD."`.

Build completed with 0 errors and 0 warnings in 0.74s.

## Verification

cargo build succeeded with 0 errors, 0 warnings. All five edits verified by exact text match and grep confirmation of Agent::Gsd references at lines 479 and 484.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo build` | 0 | ✅ pass | 760ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/app.rs`
