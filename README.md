# amux

**Agent Multiplexer** — a keyboard-first terminal UI for managing AI coding agent workspaces and sessions.

A single binary that aggregates your projects, discovers Claude Code, Codex, GSD, and OMP sessions, and lets you create, resume, and switch between agent sessions — all without leaving the terminal.

## Features

### Core

- **Multi-workspace management** — sidebar with project workspaces, expandable session lists
- **Multi-agent support** — Claude Code, Codex, GSD, and OMP, with an agent picker when creating sessions
- **PTY-embedded sessions** — agents run inside the TUI via PTY, no external windows needed
- **Session persistence** — discovers existing sessions from `~/.claude/projects`, `~/.codex/sessions`, `~/.gsd/sessions`, and `~/.omp/agent/sessions`
- **Configurable keybinds** — customize all shortcuts via `config.json`, with Alt+key mode
- **macOS + Linux** — cross-platform support with platform-aware data directories

### Session Management

- **Session tags** — tag and filter sessions
- **Session rollback** — one-click `git reset` to pre-session state with snapshot commits
- **Session chains** — define multi-step agent pipelines (Claude → Codex review) in config
- **Replay prompt** — re-use a previous session's prompt with fresh template variables
- **Session knowledge** — auto-inject workspace context into new sessions, reducing token waste
- **Pre-flight checks** — verify git state, test status, and project config before starting sessions

### Monitoring & Analysis

- **Process monitoring** — real-time CPU/memory stats for agent processes (via `/proc`)
- **Token budget alerts** — configurable daily/weekly token limits with status bar warnings
- **Token usage stats** — per-agent input/output/cost breakdown
- **Session timeline** — chronological event view across all sessions
- **Cross-session search** — full-text search across all session content
- **Semantic search** — natural language search via TF-IDF/BM25

### Remote & Web

- **Built-in HTTP server** — `amux serve` with WebSocket terminal streaming
- **Mobile-responsive web client** — xterm.js terminal, virtual function keys, agent selector
- **SSH remote discovery** — scan remote hosts for agent sessions
- **Headless CLI** — `amux run`, `amux list`, `amux status` for CI/scripting

### Development

- **Post-completion checks** — auto-run tests after session ends (Rust/Node/Python/Go)
- **Git worktree isolation** — auto-isolate conflicting PTYs in separate worktrees
- **Custom themes** — load Tokyo Night variants from JSON files
- **Plugin system** — run custom commands with ANSI output, JSON actions, lifecycle hooks
- **Project config** — `.amux.json` per-project settings (agent, check command, templates)
- **Prompt templates** — variables like `{git_diff}`, `{git_branch}`, `{project_type}`
- **Environment diagnostics** — `amux doctor` checks git, agent CLIs, data dirs

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
| `Alt+j` / `Alt+k` | Move selection |
| `h` / `l` / `Tab` | Switch between sidebar and terminal |
| `Enter` | Open or resume session |
| `Alt+n` | New session (opens agent picker) |
| `c` / `x` / `g` / `o` | Quick create Claude / Codex / GSD / OMP session |
| `Alt+r` | Refresh session list |
| `Alt+q` | Quit |
| `Alt+d` | Delete workspace or session |
| `Alt+m` | Rename workspace or session |
| `Alt+w` | Create new workspace |
| `Alt+v` | Preview session |
| `Alt+e` | Expand/collapse workspace |
| `Alt+s` | Settings |
| `Alt+t` | Theme selector |
| `Alt+f` | Filter by tag |
| `Alt+h` | Help |
| `Ctrl+J` / `Ctrl+K` | Switch between active PTY tabs |
| `PgUp` / `PgDn` | Scroll PTY output |
| `/` | Search workspaces and sessions |

All keybinds are customizable in `config.json`.

## Data Directory

**Linux:** `~/.local/share/amux/`
**macOS:** `~/Library/Application Support/amux/`

```
├── config.json          # workspace list, keybinds, settings
├── sessions/            # session metadata and tags
├── snapshots/           # git HEAD snapshots for rollback
├── summaries/           # auto-generated session summaries
├── themes/              # custom theme JSON files
├── knowledge/           # per-workspace context cache
├── embeddings/          # search index
└── workspaces/          # isolated cwd for virtual workspaces
```

## Project Config

Place `.amux.json` in your project root for per-project settings:

```json
{
  "default_agent": "claude",
  "check_command": "npm test",
  "auto_inject_knowledge": true,
  "preflight": {
    "require_clean_git": false
  }
}
```

## Documentation

- [config.json reference](docs/config.md) — all configuration fields and examples
- [Session chains](docs/chains.md) — multi-step agent pipelines

## Requirements

- Linux (x86_64) or macOS (Apple Silicon / x86_64)
- Claude Code, Codex, GSD, and/or OMP CLI installed (for agent sessions)

## License

MIT
