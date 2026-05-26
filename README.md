# Agent Workspace TUI

A keyboard-first terminal workspace switcher for local agent/code projects.

## Run

```bash
cargo run
```

By default it scans sibling directories of the current repository and treats any
directory containing `.git` as a workspace.

To control the workspace list explicitly:

```bash
AGENT_WORKSPACES="/home/huanglin/code/foo:/home/huanglin/code/bar" cargo run
```

## Keys

- `j` / `k` or arrows: move selection
- `h` / `l` or `Tab`: switch pane
- `/`: search workspaces and sessions
- `Enter`: open a shell in the selected workspace
- `q` or `Esc`: quit

## Session Sources

The first version discovers sessions from local project files:

- `.codex/sessions`
- `.gsd/activity`
- `.claude/projects`
- `.git/refs/heads`

The TUI is intentionally local-first. It does not need a daemon or GUI process
to give you a consolidated workspace/session view.
