# config.json Reference

Location: `~/.local/share/amux/config.json` (Linux) or `~/Library/Application Support/amux/config.json` (macOS).

## Top-Level Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `workspaces` | `Array<Workspace>` | `[]` | Workspace entries. Auto-populated on first run. |
| `theme` | `string` | `"dark"` | Theme: `"dark"`, `"light"`, or a custom theme name. |
| `keybinds` | `Keybinds` | (see below) | Rebindable keyboard shortcuts. |
| `templates` | `Array<SessionTemplate>` | `[]` | Saved session templates for quick launch. |
| `automations` | `Array<InputAutomation>` | `[]` | Sequenced input automations sent to PTY. |
| `archive_days` | `number \| null` | `null` | Days before sessions are auto-archived. `null` = disabled. |
| `remote_hosts` | `Array<RemoteHost>` | `[]` | SSH hosts for remote session discovery. |
| `plugins` | `Array<Plugin>` | `[]` | User-defined shell commands bound to keys or hooks. |
| `serve_port` | `number \| null` | `null` | HTTP server port. Defaults to 8080. |
| `serve_token` | `string \| null` | `null` | Bearer token for HTTP server auth. |
| `check_command` | `string \| null` | `null` | Override post-session check command (e.g. `"cargo test"`). |
| `token_budget` | `TokenBudget \| null` | `null` | Token/cost limits with alerting. |
| `chains` | `Array<SessionChain>` | `[]` | Named multi-step agent sequences. See [chains.md](chains.md). |

## Sub-Objects

### Workspace

```json
{ "id": "abc123", "name": "my-project", "path": "/home/user/code/my-project", "created_at": 1717400000 }
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | Unique identifier (auto-generated). |
| `name` | `string` | Display name. |
| `path` | `string \| null` | Filesystem path. `null` for virtual workspaces. |
| `created_at` | `number` | Unix timestamp. |

### Keybinds

All keys are rebindable. Unset fields use defaults.

```json
{
  "keybinds": {
    "move_up":      { "key": "k" },
    "move_down":    { "key": "j" },
    "quit":         { "key": "q" },
    "copy":         { "key": "y", "ctrl": true }
  }
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `move_up` | `k` | Move selection up |
| `move_down` | `j` | Move selection down |
| `expand` | `e` | Expand/collapse workspace |
| `refresh` | `r` | Refresh session list |
| `rename` | `Shift+R` | Rename workspace/session |
| `new_workspace` | `Shift+N` | Create new workspace |
| `delete` | `d` | Delete workspace/session |
| `new_session` | `n` | New session (agent picker) |
| `search` | `/` | Search |
| `help` | `?` | Help overlay |
| `settings` | `Shift+S` | Settings |
| `theme` | `Shift+T` | Cycle theme |
| `export` | `Shift+E` | Export |
| `copy` | `Ctrl+Y` | Copy to clipboard |
| `preview` | `v` | Preview session |
| `tag_filter` | `t` | Filter by tag |
| `quit` | `q` | Quit |

### SessionTemplate

```json
{ "name": "quick-fix", "agent": "Claude", "initial_prompt": "Fix all clippy warnings" }
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string` | Template display name. |
| `agent` | `"Claude" \| "Codex" \| "Omp"` | Agent type. |
| `workspace_id` | `string \| null` | Target workspace. `null` = current. |
| `initial_prompt` | `string \| null` | Prompt sent on session start. |

### InputAutomation

```json
{
  "name": "auto-approve",
  "steps": [
    { "text": "y", "delay_ms": 500 },
    { "text": "", "delay_ms": 1000 }
  ]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string` | Automation name. |
| `steps` | `Array<InputStep>` | Steps to execute. |

**InputStep**: `{ "text": "string", "delay_ms": 0 }`

### RemoteHost

```json
{
  "name": "build-server",
  "host": "10.0.0.5",
  "user": "deploy",
  "port": 2222,
  "agent_paths": ["/home/deploy/.claude/projects"]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string` | Display name. |
| `host` | `string` | Hostname or IP. |
| `user` | `string \| null` | SSH user. |
| `port` | `number \| null` | SSH port. |
| `agent_paths` | `Array<string>` | Remote paths to scan for sessions. Empty = defaults. |

### Plugin

```json
{
  "name": "run-tests",
  "command": "cd {workspace} && cargo test",
  "key": "t",
  "hooks": ["on_complete"]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string` | Plugin name. |
| `command` | `string` | Shell command. `{workspace}` and `{session_id}` are substituted. |
| `key` | `string \| null` | Single-char key binding. |
| `hooks` | `Array<string>` | Events: `"on_complete"`, `"on_idle"`. |

### TokenBudget

```json
{
  "daily_tokens": 500000,
  "weekly_tokens": 2000000,
  "daily_cost": 10.0,
  "weekly_cost": 50.0
}
```

All fields optional — set only the limits you want enforced.

## Example config.json

```json
{
  "workspaces": [],
  "theme": "dark",
  "archive_days": 30,
  "serve_port": 8080,
  "serve_token": "my-secret-token",
  "check_command": "cargo test && cargo clippy",
  "token_budget": {
    "daily_tokens": 500000,
    "daily_cost": 10.0
  },
  "templates": [
    { "name": "fix-clippy", "agent": "Claude", "initial_prompt": "Fix all clippy warnings" }
  ],
  "plugins": [
    { "name": "run-tests", "command": "cd {workspace} && cargo test", "key": "t", "hooks": ["on_complete"] }
  ],
  "remote_hosts": [
    { "name": "ci", "host": "ci.example.com", "user": "deploy" }
  ],
  "chains": []
}
```
