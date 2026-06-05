# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Tokyo Night theme preset with high-saturation accent colors
- Bottom terminal split (amux mode 'c' key) spawning $SHELL in session cwd
- Focus indicator in status bar (SIDEBAR / CHAT)
- Status bar theme slots (status_bg, status_text, status_dim) with vivid defaults
- Git diff colors in sidebar: green for insertions, red for deletions
- Virtual workspace (Pinned/Recent) expand/collapse support
- Unified theme panel selector (replaces cycle toggle)
- Tracing structured logging (tracing + tracing-subscriber)
- Default ChatMode changed to Passthrough (raw mode)
- Build check in `amux doctor` (`cargo check` in source dir)
- Config.unset_env field for user-configurable env variable removal
- Agent::ALL constant and Agent::apply_term_env() deduplication
- rayon parallel session discovery for cold-start performance
- PTY write_input backpressure with 4KB chunked writes

### Changed
- Status bar colors now fully theme-driven (no hardcoded white/black)
- CPU/MEM stats only shown in status bar when Chat is focused
- Help view cleaned up: removed duplicate keybinds, fixed descriptions
- Config struct derives Default, simplified constructors
- ProjectType derives Default with #[default] Unknown

### Fixed
- Removed dead InputMode::DiffSelect variant
- Fixed Recent workspace showing 0 sessions when collapsed
- Fixed outdated 'Press Enter to start a named Claude Code session' text
- Removed non-existent Gsd agent from docs/chains.md and docs/config.md
- watch.rs changed to NonRecursive to prevent fd exhaustion
