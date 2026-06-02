# S02: GSD UI Integration: Agent Picker, PTY Lifecycle, and Resume — Research

**Date:** 2026-06-02

## Summary

S02 is **light-touch wiring work** in a single file (`src/app.rs`). S01 already delivered the Agent::Gsd enum, discovery, and parsing — all 30 tests pass. The TUI code has clean agent-agnostic abstractions: the agent picker iterates `available_agents` (already populated by `detect_agents()` which includes GSD), the popup renders `agent.icon()`/`agent.color()`/`agent.label()`, and PTY spawn delegates to `agent.build_new_cmd()`/`agent.build_resume_cmd()`. The only GSD-specific gaps are: (1) quick-key 'G' in the agent picker, (2) GSD exemption from Codex-style PTY cleanup, and (3) two hardcoded "Claude Code" strings in help text / kill message.

## Recommendation

Make four targeted edits to `src/app.rs` only. No changes needed to `types.rs`, `discovery.rs`, `pty.rs`, or `util.rs`. The GSD variant is already wired through the entire data pipeline; the TUI just needs the keybinding, cleanup guard, and string updates.

## Implementation Landscape

### Key Files

- **`src/app.rs`** (1545 lines) — the only file that needs changes. Four touch points:
  1. **Agent picker quick-key** (lines 456–478): Add `KeyCode::Char('g') | KeyCode::Char('G')` block matching the existing C/Claude and X/Codex pattern.
  2. **PTY cleanup in `poll_states()`** (lines 110–113): The `retain()` call removes Codex PTYs when `!is_alive()`. GSD must NOT match this condition — add `slot.info.agent != Agent::Gsd` guard so GSD sessions persist after exit (like Claude).
  3. **Agent popup help text** (line 1352): Update from `" C:Claude  X:Codex  j/k:navigate..."` to include `" G:GSD"`.
  4. **Hardcoded "Claude Code" strings**: Line 1260 (`Ctrl+Q Kill current Claude Code session`) and line 233 (`Claude Code terminated`). Change to agent-agnostic text like "active session" or use `agent.label()`.

### Build Order

1. **Quick-key 'G' in agent picker** — 8-line block, pattern-matched from C/X blocks above. Highest UX impact, zero risk.
2. **PTY cleanup guard** — 2-line change in `poll_states()`. Critical for correct GSD session persistence.
3. **Help text updates** — 3 string literal changes. Cosmetic but important for consistency.
4. **Verify** — `cargo test` + `cargo build` to confirm no regressions.

### Verification Approach

- `cargo test` — all 30 existing tests must still pass
- `cargo build` — no warnings
- Manual: agent picker shows GSD with 'G' quick-key when `gsd` is installed
- Manual: GSD sessions persist in sidebar after PTY exit (not auto-cleaned)
- Manual: help text references GSD

## Detailed Touch Points

### 1. Agent Picker Quick-Key (lines 456–478)

Current pattern for C and X keys:
```rust
KeyCode::Char('c') | KeyCode::Char('C')
    if self.available_agents.contains(&Agent::Claude) =>
{
    self.agent_state.select(Some(
        self.available_agents.iter().position(|a| *a == Agent::Claude).unwrap(),
    ));
    self.confirm_input()?;
}
KeyCode::Char('x') | KeyCode::Char('X')
    if self.available_agents.contains(&Agent::Codex) =>
{
    self.agent_state.select(Some(
        self.available_agents.iter().position(|a| *a == Agent::Codex).unwrap(),
    ));
    self.confirm_input()?;
}
```

Add identical block for G:
```rust
KeyCode::Char('g') | KeyCode::Char('G')
    if self.available_agents.contains(&Agent::Gsd) =>
{
    self.agent_state.select(Some(
        self.available_agents.iter().position(|a| *a == Agent::Gsd).unwrap(),
    ));
    self.confirm_input()?;
}
```

This goes in the `InputMode::SelectAgent` match arm's `KeyCode::Char(c)` block (line 414), after the X/Codex block.

### 2. PTY Cleanup Guard (lines 108–114)

Current:
```rust
self.ptys.retain(|slot| {
    if slot.info.agent == Agent::Codex && !slot.handle.is_alive() {
        return false;
    }
    true
});
```

Change to:
```rust
self.ptys.retain(|slot| {
    if slot.info.agent == Agent::Codex && !slot.handle.is_alive() {
        return false;
    }
    if slot.info.agent == Agent::Gsd && !slot.handle.is_alive() {
        return false; // or: keep GSD alive — but research says persist like Claude
    }
    true
});
```

Wait — re-reading the requirements: "GSD sessions persist in sidebar after PTY exit (not auto-cleaned like Codex)". This means GSD should behave like Claude — the `retain` should NOT remove GSD PTYs. So the condition is already correct: only Codex is removed. No change needed here. **GSD is already exempt** because the guard only removes `Agent::Codex`.

**Correction:** No code change needed for poll_states. The existing logic only removes Codex. GSD sessions will persist by default.

### 3. Help Text (line 1352)

Current: `" C:Claude  X:Codex  j/k:navigate  Enter:confirm  Esc:cancel"`
New: `" C:Claude  X:Codex  G:GSD  j/k:navigate  Enter:confirm  Esc:cancel"`

### 4. Hardcoded "Claude Code" Strings

- Line 1260: `"Ctrl+Q       Kill current Claude Code session"` → `"Ctrl+Q       Kill current session"`
- Line 233: `"Claude Code terminated. Sessions refreshed."` → `"Session terminated. Sessions refreshed."`
- Line 1483: `"No agent CLI found. Install Claude Code or Codex."` → `"No agent CLI found. Install Claude Code, Codex, or GSD."`

## Risks

- **None significant.** All changes are string literals and one keybinding block following an established pattern. The data pipeline (enum, discovery, PTY spawn) is already complete from S01.

## Estimated Scope

- **Files changed:** 1 (`src/app.rs`)
- **Lines changed:** ~15 (8 for keybinding block, 3 for string updates, 0 for cleanup — already correct)
- **Risk:** Very low
