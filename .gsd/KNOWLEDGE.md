# Project Knowledge

## Rules

| # | Scope | Rule | Why | Added |
|---|-------|------|-----|-------|
| 1 | M001 | GSD session directories encode workspace path by replacing `/` with `-` (e.g., `/home/user/proj` → `-home-user-proj`) | Must match encoding to discover sessions per-workspace | 2026-06-02 |
| 2 | M001 | GSD JSONL v3: first line is `{type:"session", version:3, id, timestamp, cwd}` | Entry point for parsing — session ID and workspace come from here | 2026-06-02 |
| 3 | M001 | GSD titles: prefer `custom_message` with `customType:"gsd-run"`, fallback to `message` with `role:"user"` | Auto-mode uses gsd-run; interactive uses plain messages | 2026-06-02 |
| 4 | M001 | `gsd` CLI has no `--resume <id>` flag — only `gsd -c` (recent) and `gsd sessions` (interactive) | Resume specific session requires stdin pipe workaround | 2026-06-02 |
