# M001: GSD Agent Support

**Vision:** Add GSD (gsd CLI) as a first-class agent in amux, alongside Claude Code and Codex. GSD sessions are discovered from ~/.gsd/sessions/, the agent picker includes GSD with quick-key G, and users can spawn new or resume existing GSD sessions, completing the three-agent lineup.

## Success Criteria

- cargo test passes with GSD-specific unit tests (JSONL parsing, enum properties, dir name handling, session persistence)
- discover_gsd_sessions() returns sessions grouped by workspace from ~/.gsd/sessions/ (verified by unit tests + cargo run output)
- Agent::Gsd variant exposes icon G, color Magenta, label GSD, cmd gsd; included in detect_agents() when gsd CLI is installed
- Agent picker G keybinding is wired with available_agents.contains guard (verified by compilation + grep)
- build_resume_cmd() produces correct CommandBuilder with workspace CWD (verified by unit test)
- GSD sessions are not auto-cleaned after PTY exit — poll_states() retain filter only removes Agent::Codex (verified by gsd_sessions_persist_after_pty_exit test)
- When gsd CLI is not installed, detect_agents() omits Agent::Gsd (verified by which() check logic)

## Slices

- [x] **S01: GSD Agent Core: Enum, Discovery, and Session Parsing** `risk:high` `depends:[]`
  > After this: cargo test passes with GSD JSONL parsing tests, Agent::Gsd variant has correct icon/label/color/cmd/sessions_dir, discover_gsd_sessions() finds sessions from ~/.gsd/sessions/ with correct workspace grouping and title extraction

- [x] **S02: GSD UI Integration: Agent Picker, PTY Lifecycle, and Resume** `risk:low` `depends:[S01]`
  > After this: Agent picker G keybinding wired with available_agents guard. GSD sessions discovered and grouped by workspace via discover_gsd_sessions(). Resume uses build_resume_cmd() with workspace CWD. GSD sessions persist after PTY exit (poll_states only removes Codex).

## Boundary Map

Not provided.
