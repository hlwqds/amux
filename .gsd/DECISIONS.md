# Decisions Register

<!-- Append-only. Never edit or remove existing rows.
     To reverse a decision, add a new row that supersedes it.
     Read this file at the start of any planning or research phase. -->

| # | When | Scope | Decision | Choice | Rationale | Revisable? | Made By |
|---|------|-------|----------|--------|-----------|------------|---------|
| D001 | 2026-06-02 | M001 | GSD agent icon and color | "G" icon, Magenta color | Single-letter pattern matching Claude(C)/Codex(X). Magenta distinct from existing Cyan/Green. | no | agent |
| D002 | 2026-06-02 | M001 | GSD session resume mechanism | Stdin pipe to `gsd sessions` | GSD lacks --resume <id> flag. Only path to resume specific session is interactive `gsd sessions` picker. | yes — if GSD adds resume flag | agent |
| D003 | 2026-06-02 | M001 | GSD session title extraction | Prefer custom_message(gsd-run) → fallback to user message | GSD wraps auto-mode prompts in gsd-run custom messages. User messages cover interactive sessions. | no | agent |
| D004 | 2026-06-02 | M001 | GSD PTY cleanup behavior | Don't auto-remove on exit (like Claude, unlike Codex) | GSD sessions should persist for resume after PTY process completes. | no | agent |
