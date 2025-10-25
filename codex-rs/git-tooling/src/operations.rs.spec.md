## Overview
`operations.rs` centralizes low-level git interactions used across the crate. It validates repository state, normalizes paths, and wraps `git` subprocess execution with rich error reporting so higher-level helpers can compose reliable workflows.

## Detailed Behavior
- Repository helpers:
  - `ensure_git_repository` checks `rev-parse --is-inside-work-tree`, translating common exit codes into `GitToolingError::NotAGitRepository`.
  - `resolve_head` attempts `rev-parse --verify HEAD`, returning `Ok(None)` when no commits exist.
  - `resolve_repository_root` resolves the toplevel directory via `rev-parse --show-toplevel`.
  - `repo_subdir` computes a relative prefix when operating inside a subdirectory, with canonicalization fallback to handle symlinks.
- Path handling:
  - `normalize_relative_path` cleanses user-provided paths, rejecting absolute paths or segments that escape the repository (returning `PathEscapesRepository` / `NonRelativePath` errors).
  - `apply_repo_prefix_to_force_include` prepends the subdirectory prefix when force-including files from nested repos.
- Git wrappers:
  - `run_git_for_status` and `run_git_for_stdout` execute arbitrary git commands, optionally with temporary environment variables (e.g., custom index paths), and propagate detailed `GitToolingError::GitCommand` / `GitOutputUtf8`.
  - `run_git` collects arguments into an `OsString` vector, constructs a human-readable command string for diagnostics, applies environment overrides, and returns stdout/stderr plus exit status.
- Utility:
  - `cmp_by_score_desc_then_path_asc` / `sort_matches` are not present here (they live in file-search); this module just exposes `build_command_string` and `GitRun` for internal bookkeeping.

## Broader Context
- Consumed by ghost commit operations and future git helpers to avoid duplicating subprocess logic or path normalization rules.
- `GitToolingError` definitions in `errors.rs` align with these functions, so downstream crates can differentiate repository issues from command failures.

## Technical Debt
- None foregrounded; the wrapper abstracts git execution consistently for the crate’s current needs.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./ghost_commits.rs.spec.md
  - ./errors.rs.spec.md
