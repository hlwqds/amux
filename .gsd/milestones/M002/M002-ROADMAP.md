# M002: M002: Modular Refactor

**Vision:** Split the 1556-line app.rs into focused sub-modules (ui, handler, session, browse), create lib.rs for the lib/bin split, migrate 33 tests to per-module locations, and achieve zero clippy warnings with fmt compliance. Zero functional behavior changes — pure structural refactor.

## Success Criteria

- app.rs no longer exists as a single file — replaced by app/mod.rs + 4 sub-modules
- src/lib.rs exists and exposes the public API
- src/main.rs is ≤50 lines (thin entry point only)
- All 33 tests pass from per-module #[cfg(test)] locations
- cargo clippy -- -D warnings exits 0
- cargo fmt --all -- --check exits 0
- cargo build --release produces a working binary
- No new dependencies added

## Slices

- [ ] **S01: Foundation - lib.rs split and lint fixes** `risk:low` `depends:[]`
  > After this: cargo test passes all 33 tests, cargo clippy -- -D warnings exits 0, cargo fmt --all -- --check exits 0, and src/lib.rs exists as the library root with main.rs using it.

- [ ] **S02: app.rs decomposition into sub-modules** `risk:high` `depends:[S01]`
  > After this: app.rs no longer exists as a single file; replaced by app/mod.rs + app/ui.rs + app/handler.rs + app/session.rs + app/browse.rs. cargo build succeeds and cargo test passes all 33 tests.

- [ ] **S03: Test migration to per-module locations** `risk:medium` `depends:[S01,S02]`
  > After this: All 33 tests relocated from main.rs::tests to per-module #[cfg(test)] mod tests blocks. main.rs is a thin entry point (10-15 lines). cargo test passes all 33 tests from their new locations.

## Boundary Map

## Boundary Map
