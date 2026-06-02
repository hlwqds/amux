---
estimated_steps: 19
estimated_files: 2
skills_used: []
---

# T02: Implement GSD session discovery and JSONL parsing

Why: Users need to see existing GSD sessions in the sidebar. This requires scanning ~/.gsd/sessions/ directories, parsing GSD's JSONL v3 format, matching sessions to workspaces by decoding directory names, and extracting session titles.

Do:
1. Add `discover_gsd_sessions()` function in src/discovery.rs following the pattern of discover_codex_sessions(). It should:
   - Get sessions_dir from Agent::Gsd.sessions_dir(), return early if None
   - Iterate over subdirectories of ~/.gsd/sessions/ (each subdir name is an encoded workspace path: / → -)
   - For each subdir, iterate over JSONL files
   - Call parse_gsd_session() on each file
   - Match decoded workspace paths to configured workspaces
   - Build Session structs with Agent::Gsd
2. Add `parse_gsd_session()` function following parse_codex_session() pattern:
   - First line has {type:"session", version:3, id, cwd, timestamp}
   - Extract session ID from first line
   - For title: scan for custom_message with customType:"gsd-run" (per D003), extract first text content
   - Fallback: scan for message with role:"user", extract content text
   - Truncate title to 50 chars
3. Add `decode_gsd_dir_name()` helper: reverse the / → - encoding (e.g., "-home-user-proj" → "/home/user/proj")
4. Wire discover_gsd_sessions() into discover_sessions() alongside claude and codex
5. Update find_session_jsonl() Gsd arm to search ~/.gsd/sessions/<encoded-dir>/ for JSONL files

Done when: discover_sessions() returns GSD sessions when ~/.gsd/sessions/ exists with valid session files.

## Inputs

- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/discovery.rs`
- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/types.rs`

## Expected Output

- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/discovery.rs`

## Verification

cargo test
