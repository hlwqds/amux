# ADR: InputMode Consolidation

> Originally in `src/types.rs:476-537`

## Current state

36 variants (including `None`), flat C-like enum with `Copy + Clone`.

## Option A — Categorical grouping

Handler dispatch (handler.rs) uses ~30 sequential comparisons, each delegating to a dedicated method. Grouping into categories would NOT collapse these branches because each variant has unique behavior — even within the same "category", no two TextInput modes share confirm logic (SessionName → agent select, RenameSession → save title, NewWorkspaceName → browse dir, Search → filter tree).

The one genuine pattern: 8+ variants (Help, Stats, TokenStats, CrossSearch, DiffView, AgentRecommend, Timeline, ConflictWarning, BudgetWarning, KeybindView, SummaryPreview) all close on "any key" with identical dismiss. This could be a `Popup(PopupKind)` arm with a shared handler. Savings: ~30 lines of dispatch boilerplate. But each still needs its own render and cleanup logic, so the dismiss handler would need a `match PopupKind` anyway.

## Option B — Data-carrying enum

Would break `Copy` on InputMode. Every comparison throughout the codebase (30+ sites) currently works because the enum is `Copy + PartialEq`. Adding `String` or `ListState` fields means:
- Loss of `Copy` → every comparison needs `&self.input_mode` or destructuring
- Buffer state duplicated: `input_buffer` already lives on `App`. Moving it INTO InputMode creates two sources of truth.
- `confirm_input()` already reads `self.input_buffer`; if buffer were in the enum, we'd need to destructure mutably while also accessing `self.sessions`, triggering borrow conflicts.

## Option C — Keep as-is (RECOMMENDED)

The flat enum is:
1. Zero-cost: `Copy + PartialEq`, compares as a simple integer discriminant.
2. Explicit: every mode is greppable, no indirection through sub-enums.
3. Already well-factored: handler delegates to per-mode methods, UI to per-mode renders.
4. Easy to extend: adding a new mode = add one variant + one handler + one render.

Recommendation: **Keep as-is**.
Rationale: The flat enum correctly models the domain — each variant IS a distinct UI mode with unique key handling, rendering, and confirmation logic. Categorical grouping would redistribute the same match arms across more types without reducing total complexity. Data-carrying would sacrifice Copy and create borrow conflicts for no gain.
