## Overview
`find_codex_home.rs` locates the Codex configuration directory, honoring the `CODEX_HOME` environment variable or defaulting to `~/.codex`. It is a temporary copy of the helper in `codex-core` to avoid a dependency cycle.

## Detailed Behavior
- `find_codex_home`:
  - If `CODEX_HOME` is set and non-empty, returns the canonicalized path (erroring when it doesn’t exist).
  - Otherwise uses `dirs::home_dir` to construct `<home>/.codex` without verifying existence.
- Errors bubble up as `std::io::Error`, allowing callers to differentiate between missing home directories and other IO failures.

## Broader Context
- Used by OAuth storage (`oauth.rs`) when falling back to file-based credential stores. TODO comments note the desire to move this helper into a shared lower-level crate.

## Technical Debt
- Duplicated code should be consolidated once a shared dependency is available.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Move `find_codex_home` into a shared crate to remove duplication.
related_specs:
  - ../mod.spec.md
  - ./oauth.rs.spec.md
