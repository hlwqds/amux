# M001: GSD Agent Support

**Vision:** Add GSD (gsd CLI) as a first-class agent in amux, alongside Claude Code and Codex. GSD sessions appear in the sidebar, the agent picker offers GSD with quick-key G, and users can spawn new or resume existing GSD sessions, completing the three-agent lineup.

## Success Criteria

- cargo test passes with GSD-specific unit tests (JSONL parsing, enum properties, dir name handling)
- amux TUI shows existing GSD sessions in sidebar grouped by workspace when gsd CLI is installed
- Agent picker displays GSD option with quick-key G and correct icon/color
- Spawning a new GSD session from the picker launches gsd in a PTY
- Resuming a GSD session from sidebar launches gsd -c in a PTY with correct workspace CWD
- GSD sessions persist in sidebar after PTY exit (not auto-cleaned like Codex)
- When gsd CLI is not installed, no GSD UI appears and no errors occur

## Slices

- [x] **S01: GSD Agent Core: Enum, Discovery, and Session Parsing** `risk:high` `depends:[]`
  > After this: cargo test passes with GSD JSONL parsing tests, Agent::Gsd variant has correct icon/label/color/cmd/sessions_dir, discover_gsd_sessions() finds sessions from ~/.gsd/sessions/ with correct workspace grouping and title extraction

- [ ] **S02: GSD UI Integration: Agent Picker, PTY Lifecycle, and Resume** `risk:low` `depends:[S01]`
  > After this: Agent picker popup shows GSD with quick-key G and Magenta color. GSD sessions appear in sidebar. Clicking a GSD session resumes via gsd -c in PTY. GSD sessions persist after PTY exit.

## Boundary Map

Not provided.
