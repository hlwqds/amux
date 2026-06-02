---
phase: complete
phase_name: M001 — GSD Agent Support
project: amux
generated: 2026-06-02T12:00:00Z
counts:
  decisions: 4
  lessons: 3
  patterns: 3
  surprises: 2
missing_artifacts: none
---

# M001 Learnings

### Decisions

- **D001: GSD icon "G" + Magenta color.** Single-letter pattern matching Claude(C)/Codex(X). Magenta distinct from existing Cyan/Green. Non-revisable.
  Source: DECISIONS.md/D001

- **D002: Resume via `gsd -c` (most recent session).** GSD CLI lacks `--resume <id>` flag, so amux can only resume the most recent session via `gsd -c`. Revisable if GSD adds a resume flag.
  Source: DECISIONS.md/D002

- **D003: Title extraction: custom_message(gsd-run) → fallback user message.** GSD wraps auto-mode prompts in gsd-run custom messages. User messages cover interactive sessions.
  Source: DECISIONS.md/D003

- **D004: GSD sessions persist after PTY exit.** Unlike Codex which auto-cleans, GSD sessions should remain discoverable for later resume, matching Claude Code behavior.
  Source: DECISIONS.md/D004

### Lessons

- **GSD directory encoding is lossy.** GSD encodes workspace paths by replacing `/` with `-`, meaning paths containing hyphens in the original path may decode incorrectly or collide. This is a known limitation of GSD's own encoding scheme, not introduced by amux. The codebase uses a simple replace scheme matching GSD's behavior. Root cause: GSD chose a reversible-unless-hyphens encoding. Fix: accepted as known limitation, documented in S01 SUMMARY.
  Source: S01-SUMMARY.md/Known limitations

- **Task planning overreach: T02 was a no-op.** T02 in S01 (GSD session discovery wiring) required no code changes because T01 (Agent::Gsd enum + helpers) had already implemented all T02 deliverables. Earlier tasks absorbed downstream scope. Root cause: task boundaries were too granular for the actual implementation pattern. Fix: accepted as deviation, no code change needed.
  Source: S01-SUMMARY.md/Deviations

- **GSD CLI resume limitations constrain UX.** The gsd CLI has no `--resume <session-id>` flag — only `gsd -c` for most recent and `gsd sessions` for interactive picker. This means amux can only resume the most recent GSD session, not target a specific one. Root cause: gsd CLI API surface. Fix: use `gsd -c`, document as known limitation.
  Source: S02-SUMMARY.md/Known limitations

### Patterns

- **Agent enum extension pattern.** To add a new agent: add variant to `Agent` enum, then implement all helper methods (cmd, label, icon, color, sessions_dir, build_new_cmd, build_resume_cmd) in single match blocks. Keeps exhaustiveness checkable.
  Source: S01-SUMMARY.md/Patterns established

- **Session discovery pattern.** Per-agent scanner: scan agent-specific sessions directory, parse agent-specific JSONL format, match to workspaces by decoding directory names. Isolated per-agent logic, composable in discover_all_sessions().
  Source: S01-SUMMARY.md/Patterns established

- **Agent picker guarded keybinding pattern.** KeyCode match + available_agents.contains() check ensures keybindings only fire when the corresponding CLI is installed. Consistent across all three agents (C/Claude, X/Codex, G/GSD).
  Source: S02-SUMMARY.md/Patterns established

### Surprises

- **GSD JSONL v3 parsing was simpler than expected.** The v3 format uses well-structured single-line session headers with `{type: "session", version: 3}`, making parsing straightforward compared to more complex formats anticipated during research.
  Source: S01-SUMMARY.md/What Happened

- **The `which()` detection approach is gracefully degrading.** Using `which("gsd")` for agent detection means GSD-specific UI simply doesn't appear when the CLI isn't installed — no runtime errors, no feature flags needed. Same pattern works for all three agents.
  Source: S01-SUMMARY.md/What Happened
