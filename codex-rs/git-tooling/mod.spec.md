## Overview
`codex-git-tooling` wraps git command-line interactions that Codex relies on for snapshotting worktrees, restoring temporary commits, and creating symlinks in cross-platform environments. It exposes higher-level helpers (ghost commits, repo resolution, command wrappers) and a shared error type so other crates can manage git state safely.

## Detailed Behavior
- `lib.rs` re-exports the public surface (`GitToolingError`, ghost commit helpers, `create_symlink`) and defines the `GhostCommit` value object used to reference transient commits.
- `ghost_commits.rs` builds and restores "ghost" commits:
  - Captures working tree snapshots into an isolated index, honors force-include paths, and fabricates default author metadata.
  - Provides options (`CreateGhostCommitOptions`) for customizing messages and inclusion lists, plus helpers to restore snapshots either by `GhostCommit` or raw commit ID.
  - Includes regression tests covering repos without HEADs, subdirectory restores, ignored files, and validation of force-include paths.
- `operations.rs` centralizes git command execution:
  - Validates repositories, resolves heads/root paths, normalizes relative paths, and wraps `git` invocations returning either status or stdout.
  - Tracks command strings for error reporting and protects against directory escapes when computing prefixes.
- `errors.rs` defines `GitToolingError`, unifying git failures, UTF-8 issues, repo validation errors, and IO/walkdir problems.
- `platform.rs` exposes `create_symlink`, dispatching to the appropriate OS-specific symlink APIs (Unix vs. Windows) with a compile-time guard for other platforms.

## Broader Context
- Ghost commit functionality underpins Codex snapshot/restore flows referenced by core execution handlers (`core/src/tools/handlers/apply_patch.rs.spec.md`) and by automation that needs reversible workspace changes.
- `GitToolingError` surfaces throughout Codex orchestration when git operations fail, making error handling consistent across crates (`core` and service binaries).

## Technical Debt
- None noted; the crate’s abstractions are narrow and focused on git CLI interoperability.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/ghost_commits.rs.spec.md
  - ./src/operations.rs.spec.md
  - ./src/errors.rs.spec.md
  - ./src/platform.rs.spec.md
