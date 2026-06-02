# Project

## What This Is

**amux** — Agent Multiplexer, a keyboard-first terminal UI for managing AI coding agent workspaces and sessions. A single Rust binary that aggregates projects, discovers agent sessions (Claude Code, Codex, and GSD), and lets users create, resume, and switch between sessions — all without leaving the terminal.

## Core Value

Multi-agent session management in a single TUI — spawn, resume, and switch between Claude Code, Codex, and GSD sessions from one interface.

## Project Shape

- **Complexity:** simple
- **Why:** Three-agent architecture is complete. All agents follow the same enum/discovery/PTY pattern. Well-defined seams, no architectural unknowns.

## Current State

Working TUI with Claude Code, Codex, and GSD support. Session discovery, spawn, resume, rename, delete all functional. PTY-embedded sessions with scrollback. 33 unit tests passing. CI via GitHub Actions. No tests in CI pipeline (tests exist in main.rs only).

## Architecture / Key Patterns

- **Language:** Rust, edition 2024
- **TUI framework:** ratatui + crossterm
- **PTY:** portable-pty + vt100 parser
- **Agent pattern:** `Agent` enum with helper methods for CLI cmd, label, icon, color, sessions_dir, build_new_cmd, build_resume_cmd
- **Session discovery:** per-agent scanner in `discovery.rs` reading JSONL files from agent-specific directories
- **Config:** JSON in `~/.local/share/amux/`, XDG-compliant
- **Data flow:** App holds workspaces + sessions + active PTYs; sidebar renders tree; PTY output goes through vt100 parser → tui-term widget

## Capability Contract

See `.gsd/REQUIREMENTS.md` for the explicit capability contract, requirement status, and coverage mapping.

## Milestone Sequence

- [x] M001: GSD Agent Support — Add GSD (gsd CLI) as a first-class agent alongside Claude Code and Codex