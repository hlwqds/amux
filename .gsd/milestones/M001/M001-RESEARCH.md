# M001: GSD Agent Support — Research

**Date:** 2026-06-02

## Summary

Adding GSD as a third agent to amux follows the exact same patterns already established for Claude Code and Codex. The codebase has a clean agent abstraction: an `Agent` enum in `types.rs` with methods for command building and session directory discovery, plus a `discover_*_sessions()` function pattern in `discovery.rs`. GSD integration requires adding one enum variant and one discovery function, plus wiring in the agent picker and PTY lifecycle.

The primary technical risk is GSD session resume — `gsd` has no `--resume <id>` flag. The context brief proposes piping a session number via stdin to `gsd sessions`, but this is fragile (listing order must match). The safer alternative is `gsd -c` (resume most recent) combined with the session directory's `cwd` field to scope which sessions appear per-workspace.

## Recommendation

Add the `Agent::Gsd` variant with icon "G" and color Magenta, following the established pattern exactly. For session discovery, scan `~/.gsd/sessions/*/` for JSONL files and extract `cwd` from the session record (line 1) rather than trying to decode directory names. For resume, use `gsd -c` with the workspace directory as CWD — this resumes the most recent session *for that directory*, which is the correct behavior when the user selects a session from a specific workspace. Skip the `gsd sessions` stdin-pipe approach; it's fragile and `gsd -c` already provides directory-scoped resume.

## Implementation Landscape

### Key Files

- **`src/types.rs`** — `Agent` enum: add `Gsd` variant with `cmd()`, `label()`, `icon()`, `color()`, `build_new_cmd()`, `build_resume_cmd()`, `sessions_dir()` methods. New `GsdRecord` / `GsdMessage` deserialization structs.
- **`src/discovery.rs`** — Add `discover_gsd_sessions()` function and `parse_gsd_session()` / `extract_gsd_title()` helpers. Wire into `discover_sessions()` and `find_session_jsonl()`.
- **`src/util.rs`** — `detect_agents()`: add `which("gsd")` check for `Agent::Gsd`.
- **`src/app.rs`** — Agent picker: add `'g'`/`'G'` quick-key binding. PTY cleanup in `poll_states()`: GSD should NOT be auto-removed on exit (like Claude, unlike Codex). Help text and error messages: update to include GSD. `render_agent_popup()`: update help line.
- **`src/main.rs`** — Tests: add `parse_gsd_session_valid`, `parse_gsd_session_invalid_json`, `gsd_agent_traits` unit tests. Update `agent_traits` test.
- **`src/config.rs`** — `encode_project_path()` already exists (used for Claude); GSD discovery won't need it since we read `cwd` from JSONL directly.

### Build Order

1. **`src/types.rs`** first — `Agent::Gsd` variant with all match arms. Everything else depends on this compiling.
2. **`src/util.rs`** — `detect_agents()` addition. Simple one-liner.
3. **`src/discovery.rs`** — GSD session discovery and JSONL parsing. This is the substantive new logic.
4. **`src/main.rs`** — Unit tests for GSD JSONL parsing.
5. **`src/app.rs`** — Agent picker wiring, PTY cleanup exemption, UI text updates.

### Verification Approach

- `cargo test` passes with new GSD tests (JSONL parsing, enum properties)
- `cargo build` compiles without warnings
- Manual: `gsd` installed → GSD appears in agent picker with quick-key G
- Manual: existing GSD sessions visible in sidebar under correct workspaces
- Manual: new GSD session spawns in PTY
- Manual: GSD session resume works via `gsd -c` with workspace CWD
- Manual: `gsd` not installed → no GSD UI shown, no errors

## GSD Session Format Details

### Directory Structure
```
~/.gsd/sessions/
  --home-huanglin-code-wuxia--/
    2026-06-02T05-53-58-213Z_019e86e5-59c5-7bb5-b922-4cdf373cf151.jsonl
  --home-huanglin-code-agent-workspace-tui--/
    2026-06-02T09-28-33-534Z_019e87a9-cffe-7d09-b57d-a8169d02794a.jsonl
```

Directory names encode the workspace path (slashes → dashes, wrapped in dashes), but the encoding is **lossy** (can't distinguish `-` in path from separator). **Do not decode directory names** — instead read `cwd` from the session record (line 1 of each JSONL).

### JSONL Record Types (v3)
```jsonl
{"type":"session","version":3,"id":"019e86e5-...","timestamp":"2026-06-02T05:53:58.213Z","cwd":"/home/huanglin/code/wuxia"}
{"type":"message","id":"...","parentId":"...","timestamp":"...","message":{"role":"user","content":[{"type":"text","text":"user prompt here"}]}}
{"type":"custom_message","id":"...","parentId":"...","timestamp":"...","customType":"gsd-run","content":"...","display":false}
{"type":"message","id":"...","parentId":"...","timestamp":"...","message":{"role":"assistant","content":[{"type":"text","text":"response"}]}}
{"type":"model_change","id":"...","parentId":"...","timestamp":"...","provider":"zhipu-cn","modelId":"glm-5.1"}
```

### Title Extraction Strategy
1. **Prefer** `custom_message` with `customType: "gsd-run"` → use `content` field (truncated to 50 chars)
2. **Fallback** `message` with `role: "user"` → use `message.content[0].text` (truncated to 50 chars, apply `clean_user_message()`)

### GSD CLI Commands
- **New session**: `gsd` (no args, interactive) — CWD determines session directory
- **Resume most recent**: `gsd -c` — resumes most recent session for current directory
- **Resume specific**: `gsd sessions` — interactive numbered picker (CWD-scoped)
- **No `--resume <id>` flag exists**

## Constraints

- **No new crate dependencies** — all needed crates already in Cargo.toml (serde, serde_json)
- **Rust edition 2024** — let-chains (`if let ... && let ...`) are available
- **Linux x86_64 target** — no Windows/macOS PTY concerns
- **Must not break existing Claude/Codex** — all existing tests must pass

## Common Pitfalls

- **GSD directory name decoding is lossy** — paths containing `-` (like `agent-workspace-tui`) can't be round-tripped through the encoding. Always use the `cwd` field from the session JSONL record instead.
- **`gsd sessions` is CWD-scoped** — it only shows sessions started from the current directory. Running it from the workspace path will show the right sessions; running from elsewhere won't. This is actually an advantage for the `gsd -c` resume approach.
- **GSD auto-mode sessions have empty user message text** — the first `message` with `role: "user"` may have empty `content[0].text`. Always check for `gsd-run` custom_message first for auto-mode sessions.
- **Root-level sessions** — sessions started from `~/.gsd/sessions/` (not in a workspace subdir) have no workspace directory prefix. These should be matched purely by `cwd` from the session record.
- **Codex auto-cleanup** — `poll_states()` removes Codex PTYs when `!is_alive()`. GSD must NOT match this condition (should behave like Claude, persisting after exit).

## Open Risks

- **`gsd -c` resumes most recent for CWD** — if the user clicks an older session in the sidebar, `gsd -c` will still resume the most recent one for that directory. This is a known limitation documented in the context. The alternative (stdin pipe to `gsd sessions`) is more fragile. Worth flagging to the user but not blocking.
- **GSD JSONL format stability** — v3 is current but could change. Discovery breaks silently (sessions don't appear, no crash). Low risk for near-term.
