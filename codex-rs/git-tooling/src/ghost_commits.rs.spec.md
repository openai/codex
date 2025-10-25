## Overview
`ghost_commits.rs` manages creation and restoration of “ghost” commits—ephemeral snapshots of a repository’s working tree that can be applied without touching the live index. It exposes ergonomic options for capturing state, handles subdirectory scoping, and ensures ignored files can be force-included when needed.

## Detailed Behavior
- `CreateGhostCommitOptions`:
  - Carries the target repo path, optional commit message, and a list of paths to force-include even if ignored.
  - Provides builder helpers (`new`, `message`, `force_include`, `push_force_include`) to compose options ergonomically.
- `create_ghost_commit`:
  - Validates the path is inside a git repo, resolves the toplevel directory and current HEAD.
  - Normalizes and prefixes force-include paths relative to the repo root to thwart directory escape attempts.
  - Creates a temporary index (`GIT_INDEX_FILE`) so changes can be staged without mutating the user’s index.
  - Runs `git add --all` scoped to the working subdir, applies force-included paths with `--force`, writes a tree, and commits it via `git commit-tree` using default Codex snapshot identity (unless HEAD is absent, in which case the commit has no parent).
  - Returns a `GhostCommit` capturing the new commit ID and optional parent.
- `restore_ghost_commit` / `restore_to_commit`:
  - Ensure the target path is a git repo, compute the relative repo prefix, and execute `git restore --worktree --staged --source <commit>` (scoped to the subdirectory or repository root).
  - Preserve ignored files and untouched siblings in parent directories.
- Internal helpers include `default_commit_identity` (snapshot signature) and a comprehensive test suite covering round-trip capture, repositories without HEAD, custom messages, invalid force-includes, and subdirectory restores.

## Broader Context
- Used by Codex orchestration to snapshot workspaces before running risky operations (e.g., apply-patch pipelines) so changes can be reverted quickly.
- Relies on command wrappers in `operations.rs` to keep git invocation logic consistent (`run_git_for_status`, `run_git_for_stdout`).
- Works hand-in-hand with `GhostCommit`’s lightweight struct exported from `lib.rs`.

## Technical Debt
- None noted; validation, identity defaults, and restore semantics are well-covered by tests.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./operations.rs.spec.md
  - ./errors.rs.spec.md
