---
estimated_steps: 12
estimated_files: 2
skills_used: []
---

# T01: Add Agent::Gsd enum variant with all helper methods

Why: Foundation for all GSD support — every other feature depends on the Agent enum having a Gsd variant.

Do:
1. Add `Gsd` to the `Agent` enum in src/types.rs alongside Claude and Codex
2. Add `Gsd => "gsd"` arm to cmd()
3. Add `Gsd => "GSD"` arm to label()
4. Add `Gsd => "G"` arm to icon()
5. Add `Gsd => Color::Magenta` arm to color()
6. Add `Gsd` arm to build_new_cmd(): create CommandBuilder for "gsd", set workspace CWD, set TERM=xterm-256color, remove KITTY/GHOSTTY env vars (follow Claude pattern). No -n flag needed for GSD.
7. Add `Gsd` arm to build_resume_cmd(): create CommandBuilder for "gsd" with args ["sessions"], set workspace CWD, same env setup. Per D002, resume uses `gsd sessions` interactive picker, not a --resume flag.
8. Add `Gsd` arm to sessions_dir(): return `~/.gsd/sessions` if it exists, else None (follow Codex pattern)
9. Add `Gsd` arm to find_session_jsonl() in discovery.rs — walk ~/.gsd/sessions/ subdirs looking for JSONL containing session ID

Done when: cargo test passes; Agent::Gsd variant compiles and all match arms are exhaustive.

## Inputs

- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/types.rs`
- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/discovery.rs`

## Expected Output

- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/types.rs`
- `/home/huanglin/.gsd/projects/c369b600c4a1/worktrees/M001/src/discovery.rs`

## Verification

cargo test
