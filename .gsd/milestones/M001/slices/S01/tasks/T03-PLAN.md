---
estimated_steps: 16
estimated_files: 3
skills_used: []
---

# T03: Wire GSD detection into detect_agents() and add comprehensive unit tests

Why: GSD must be detected via `which("gsd")` to appear in the available agents list, and all new code needs test coverage (R009).

Do:
1. In src/util.rs detect_agents(), add: `if which("gsd").is_some() { agents.push(Agent::Gsd); }` after the codex check
2. Add unit tests in src/main.rs test module:
   - agent_gsd_traits: verify Agent::Gsd.cmd()=="gsd", .label()=="GSD", .icon()=="G", .color()==Color::Magenta
   - encode_decode_gsd_dir_roundtrip: encode path → decode → assert equal
   - decode_gsd_dir_name_simple: "-home-user-proj" → "/home/user/proj"
   - decode_gsd_dir_name_root: "-" → "/"
   - parse_gsd_session_valid: create temp JSONL with v3 session header + custom_message(gsd-run) + user message, verify id/title/cwd
   - parse_gsd_session_title_from_user_message: JSONL with no gsd-run but user message, verify fallback title
   - parse_gsd_session_title_from_gsd_run_preferred: JSONL with both gsd-run and user message, verify gsd-run wins
   - parse_gsd_session_empty_file: empty JSONL returns None
   - parse_gsd_session_no_session_header: JSONL without session type returns None
   - discover_gsd_sessions_finds_by_workspace: create temp ~/.gsd/sessions/<encoded-dir>/ with valid JSONL, verify session appears for matching workspace
3. Verify existing tests still pass (no regressions)

Done when: `cargo test` passes with all new GSD tests green; existing Claude/Codex tests still pass.

## Inputs

- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/util.rs`
- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/main.rs`
- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/discovery.rs`

## Expected Output

- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/util.rs`
- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/main.rs`

## Verification

cargo test
