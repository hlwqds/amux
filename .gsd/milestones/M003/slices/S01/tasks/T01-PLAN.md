---
estimated_steps: 3
estimated_files: 2
skills_used: []
---

# T01: Add code-fuzzy-match dependency and InputMode::Search variant

1. Add `code-fuzzy-match = "0.2"` to Cargo.toml dependencies
2. Add `Search` variant to `InputMode` enum in `src/types.rs`
3. Run `cargo check` to verify compilation

## Inputs

- `Cargo.toml`
- `src/types.rs`

## Expected Output

- `Cargo.toml with code-fuzzy-match dep`
- `src/types.rs with InputMode::Search`

## Verification

cargo check
