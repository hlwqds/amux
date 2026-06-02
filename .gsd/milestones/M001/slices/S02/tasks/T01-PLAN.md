---
estimated_steps: 35
estimated_files: 1
skills_used: []
---

# T01: Add GSD quick-key, help text, and agent-agnostic strings to app.rs

Why: The TUI agent picker needs a GSD quick-key binding and all hardcoded "Claude Code" strings need to be agent-agnostic so the three-agent lineup is complete.

Do: Make five targeted edits to src/app.rs only:

1. **Agent picker quick-key 'G'** (in `handle_agent_key()`, after the X/Codex block around line 470): Add a new match arm:
```rust
KeyCode::Char('g') | KeyCode::Char('G')
    if self.available_agents.contains(&Agent::Gsd) =>
{
    self.agent_state.select(Some(
        self.available_agents
            .iter()
            .position(|a| *a == Agent::Gsd)
            .unwrap(),
    ));
    self.confirm_input()?;
}
```
This follows the exact pattern of the C/Claude and X/Codex blocks above it.

2. **Help text in agent picker** (line 1352): Change:
`" C:Claude  X:Codex  j/k:navigate  Enter:confirm  Esc:cancel"`
to:
`" C:Claude  X:Codex  G:GSD  j/k:navigate  Enter:confirm  Esc:cancel"`

3. **Kill status message** (line 233): Change:
`"Claude Code terminated. Sessions refreshed."`
to:
`"Session terminated. Sessions refreshed."`

4. **Help panel kill line** (line 1260): Change:
`"Ctrl+Q       Kill current Claude Code session"`
to:
`"Ctrl+Q       Kill current session"`

5. **No-agent error** (line 1483): Change:
`"No agent CLI found. Install Claude Code or Codex."`
to:
`"No agent CLI found. Install Claude Code, Codex, or GSD."`

No changes needed to PTY cleanup in poll_states() — the existing retain() only removes Agent::Codex, so GSD sessions persist after PTY exit by default.

Done when: All five edits applied, cargo build succeeds with 0 warnings.

## Inputs

- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/app.rs`
- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/types.rs`

## Expected Output

- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/app.rs`

## Verification

cargo build
