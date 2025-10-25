## Overview
`lib.rs` stitches together the git-tooling crate’s public API. It re-exports ghost commit helpers, the error type, and the platform-aware symlink factory, while encapsulating the lightweight `GhostCommit` wrapper used throughout Codex.

## Detailed Behavior
- Declares module structure (`errors`, `ghost_commits`, `operations`, `platform`) and publicly re-exports:
  - `GitToolingError` so callers can match on detailed git failures.
  - Ghost commit construction/restoration helpers and option struct.
  - `create_symlink` for cross-platform symlink creation.
- Defines `GhostCommit`:
  - Holds the created commit ID plus its optional parent.
  - Provides constructors/getters (`new`, `id`, `parent`) and implements `Display` for easy logging.

## Broader Context
- Consumed by higher-level orchestration code (`core::git_info`, apply-patch tooling) to snapshot worktrees and revert changes safely.
- Keeps the crate’s surface cohesive so downstream crates import from `codex_git_tooling` without referencing module internals.

## Technical Debt
- None identified; the module is a straightforward facade over the internal helpers.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./ghost_commits.rs.spec.md
  - ./operations.rs.spec.md
  - ./platform.rs.spec.md
