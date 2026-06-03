# amux

**Agent Multiplexer** — a keyboard-first terminal UI for managing AI coding agent workspaces and sessions.

A single binary that aggregates your projects, discovers Claude Code, Codex, GSD, and OMP sessions, and lets you create, resume, and switch between agent sessions — all without leaving the terminal.

## Features

- **Multi-workspace management** — sidebar with project workspaces, expandable session lists
- **Multi-agent support** — Claude Code, Codex, GSD, and OMP, with an agent picker when creating sessions
- **PTY-embedded sessions** — agents run inside the TUI via PTY, no external windows needed
- **Session persistence** — discovers existing sessions from `~/.claude/projects`, `~/.codex/sessions`, `~/.gsd/sessions`, and `~/.omp/agent/sessions`
- **Virtual workspaces** — create workspaces not bound to any local directory
- **Renaming** — rename workspaces and sessions inline
- **Config persistence** — platform-aware data directory (`~/.local/share/amux/` on Linux, `~/Library/Application Support/amux/` on macOS)

## Install

Download from [GitHub Releases](https://github.com/hlwqds/amux/releases):

```bash
tar xzf amux-x86_64-linux.tar.gz    # Linux
# or: tar xzf amux-aarch64-macos.tar.gz    # macOS
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

## CLI Usage

### Diagnostics

```bash
amux doctor                # check environment (git, agent CLIs, data dir)
```

### Headless Mode

```bash
amux run --agent claude --prompt "fix bug" --workspace ./proj   # run agent non-interactively
amux run --agent claude --prompt "refactor" --timeout 300       # with timeout (seconds)
amux list                  # list all sessions
amux list --json           # list sessions as JSON
amux status <session-id>   # show single session status
```

### HTTP Server

```bash
amux serve --port 8080 --token my-secret    # start HTTP+WebSocket server
```

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
| `c` / `x` / `g` / `o` | Quick create Claude / Codex / GSD / OMP session |
| `1` / `2` / `3` / `4` | Filter sessions by agent type (Claude/Codex/GSD/OMP) |
| `s` | Cycle sort mode |
| `Ctrl+J` / `Ctrl+K` | Switch between active PTY tabs |
| `/` | Search workspaces and sessions |
| `q` | Quit |

## Data Directory

**Linux:** `~/.local/share/amux/`
**macOS:** `~/Library/Application Support/amux/`

```
├── config.json          # workspace list and settings
├── sessions/            # session title overrides
└── workspaces/          # isolated cwd for virtual workspaces
```

## Documentation

- [config.json reference](docs/config.md) — all configuration fields and examples
- [Session chains](docs/chains.md) — multi-step agent pipelines

## Requirements

- Linux (x86_64) or macOS (Apple Silicon / x86_64)
- Claude Code, Codex, GSD, and/or OMP CLI installed (for agent sessions)

## License

MIT
