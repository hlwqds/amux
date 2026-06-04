# amux

**Agent Multiplexer** — a keyboard-first terminal UI for managing AI coding agent workspaces and sessions.

A single binary that aggregates your projects, discovers Claude Code, Codex, GSD, and OMP sessions, and lets you create, resume, and switch between agent sessions — all without leaving the terminal.

## Features

- **Dual input mode** — F12 toggles between Passthrough (RAW) and Amux (command) mode
- **Catppuccin Mocha** theme (default), plus Dark, Light, and custom JSON themes
- **alacritty_terminal backend** — full PTY emulation with scrollback search
- **Multi-agent support** — Claude Code, Codex, GSD, and OMP with agent picker
- **Session persistence** — auto-discovers sessions from agent data directories
- **Configurable keybinds** — customize all Alt+key shortcuts via `config.json`
- **macOS + Linux** — cross-platform with platform-aware data directories

## Keybindings

### Global (both modes)

| Key | Action |
|-----|--------|
| `F12` | Toggle Passthrough ↔ Amux mode |
| `Tab` / `Alt+H` | Go to sidebar |
| `Alt+J/K` | Move selection |
| `Alt+N` | New session (agent picker) |
| `Alt+R` | Refresh session list |
| `Alt+Q` | Quit |
| `Alt+D` | Delete workspace/session |
| `Alt+M` | Rename |
| `Alt+W` | Create workspace |
| `Alt+V` | Preview session |
| `Alt+E` | Expand/collapse workspace |
| `Alt+S` | Settings |
| `Alt+T` | Theme selector |
| `Alt+F` | Filter by tag |
| `Alt+L` | Focus chat |
| `Ctrl+J` / `Ctrl+K` | Switch PTY tabs |
| `Ctrl+Shift+J/K` | Reorder PTY tabs |
| `Ctrl+Q` | Kill current session |
| `Ctrl+Y` | Copy session title |

### Amux Mode (command mode, letters = actions)

| Key | Action |
|-----|--------|
| `b` | Scrollback page up |
| `f` | Scrollback search |
| `t` | Token usage |
| `s` | Activity stats |
| `e` | Chain select |
| `g` | Session timeline |
| `w` | Agent recommendations |
| `r` | Remote sessions |
| `x` | Diff view |
| `y` | Copy visible screen (when scrolled) |
| `PgUp`/`PgDn` | Scroll PTY output |
| `Home`/`End` | Scroll to top/bottom |

Modified keys (`Ctrl+X`, `Alt+X`, `Shift+X`) still forward to PTY.

### Passthrough Mode (RAW, default for typing)

All keys are forwarded directly to the PTY (agent session). Use this for normal agent interaction.

### Sidebar

| Key | Action |
|-----|--------|
| `c` / `x` / `o` | Quick create Claude/Codex/OMP session |
| `1` / `2` / `3` | Filter by agent type |
| `Space` | Mark/unmark session |
| `s` | Cycle sort mode |
| `S` | Semantic search (BM25) |
| `o` | Open workspace directory |
| `!` | Pin/unpin session |
| `p` | Template select |
| `B` | Git branch |
| `G` | Toggle archived sessions |

All keybinds are customizable in `config.json`.

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
