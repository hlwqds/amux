# Contributing to amux

Thanks for your interest in contributing! This guide covers the basics.

## Development Setup

### Prerequisites
- Rust 1.85+ (`rustup update stable`)
- OpenSSL development libraries (`libssl-dev` on Debian/Ubuntu)
- Git

### Build & Run
```bash
git clone https://github.com/nicepkg/amux.git
cd amux
cargo build
cargo run
```

### Run Tests
```bash
cargo test
```

### Lint
```bash
cargo clippy -- -D warnings
cargo fmt --check
```

## Commit Convention

Use [Conventional Commits](https://www.conventionalcommits.org/):
- `feat:` new feature
- `fix:` bug fix
- `docs:` documentation only
- `refactor:` code restructuring without behavior change
- `test:` adding or updating tests
- `chore:` build, CI, tooling changes

## Pull Request Process
1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Make your changes with clear commit messages
4. Ensure `cargo clippy -- -D warnings` and `cargo test` pass
5. Open a PR with a description of the change and motivation

## Code Style
- Follow `rustfmt` defaults (`cargo fmt`)
- Resolve all clippy warnings before committing
- Write tests for new functionality
- Keep functions focused and small

## Reporting Issues
- Use GitHub Issues
- Include: OS, Rust version, steps to reproduce, expected vs actual behavior
