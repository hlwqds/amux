# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.2] - 2026-06-17

### Added
- `Alt+l` shortcut: sidebar → chat (symmetric with `Alt+h` chat → sidebar)
- `is_idle()` method on PtyHandle for "thinking" UI indication
- `tree_index_for_pty()` for sidebar cursor sync (ActiveTab + Session nodes)

### Changed
- Tab key is now mode-aware: Passthrough passes through to PTY for completion, Amux switches sidebar
- Completion state (`PtyState::Completed`) is now determined solely by process liveness, not output idleness
- `PtyHandle::resize()` skips work when dimensions are unchanged (eliminates per-frame grid reallocation)
- OMP sessions now inject `PI_TUI_RESIZE_IN_PLACE=0` for compatibility with nested PTY rendering
- Help popup text updated to reflect new keybindings (Alt+h/Alt+l, Tab mode behavior)

### Fixed
- Tab completion broken in agent programs (Claude Code, OMP) — Tab was unconditionally intercepted to switch sidebar
- Status flapping between Running and Completed caused by 3-second idle timeout acting as completion proxy
- Spurious "completed" desktop notifications fired on every Running→Completed transition
- Ctrl+J/K switching tabs did not sync sidebar cursor (especially for resumed sessions with session_id)
- OMP Ctrl+O expand overlay flashed and immediately reverted
- CI failures: clippy 1.96 `unnecessary_sort_by` lint, `doc_lazy_continuation`, `field_reassign_with_default`
- Release workflow blocked by `cargo publish` when `CARGO_REGISTRY_TOKEN` is absent
