# amux

**Agent Multiplexer** — a keyboard-first terminal UI for managing AI coding agent workspaces and sessions.

A single binary that aggregates your projects, discovers Claude Code and Codex sessions, and lets you create, resume, and switch between agent sessions — all without leaving the terminal.

## Features

- **Multi-workspace management** — sidebar with project workspaces, expandable session lists
- **Multi-agent support** — Claude Code and Codex, with an agent picker when creating sessions
- **PTY-embedded sessions** — agents run inside the TUI via PTY, no external windows needed
- **Session persistence** — discovers existing sessions from `~/.claude/projects` and `~/.codex/sessions`
- **Virtual workspaces** — create workspaces not bound to any local directory
- **Renaming** — rename workspaces and sessions inline
- **Config persistence** — XDG-compliant data directory (`~/.local/share/amux/`)

## Install

Download from [GitHub Releases](https://github.com/hlwqds/amux/releases):

```bash
tar xzf amux-x86_64-linux.tar.gz
chmod +x amux
sudo mv amux /usr/local/bin/
```

Or build from source:

```bash
cargo build --release
cp target/release/amux /usr/local/bin/
```

## Usage

```bash
amux
```

On first run, amux auto-discovers git repositories in the parent directory of your current working directory. You can also create workspaces manually.

## Keybindings

| Key | Action |
|-----|--------|
| `j` / `k` / arrows | Move selection |
| `h` / `l` / `Tab` | Switch between sidebar and terminal |
| `Enter` | Open or resume session |
| `N` | Create new workspace |
| `D` | Delete workspace or session |
| `R` | Rename workspace or session |
| `n` | New session (opens agent picker) |
| `c` / `x` | Quick create Claude / Codex session |
| `Ctrl+J` / `Ctrl+K` | Switch between active PTY tabs |
| `/` | Search workspaces and sessions |
| `q` | Quit |

## Data Directory

```
~/.local/share/amux/
├── config.json          # workspace list and settings
├── sessions/            # session title overrides
└── workspaces/          # isolated cwd for virtual workspaces
```

## Requirements

- Linux x86_64
- Claude Code CLI and/or Codex CLI installed (for agent sessions)

## License

MIT
