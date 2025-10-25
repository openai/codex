## Overview
`platform.rs` provides the crate’s cross-platform symlink helper. It hides the platform-specific syscalls needed to replicate git’s symlink behavior when restoring worktrees.

## Detailed Behavior
- On Unix (`cfg(unix)`):
  - `create_symlink` ignores the `source` parameter (only used on Windows) and calls `std::os::unix::fs::symlink(link_target, destination)`.
- On Windows (`cfg(windows)`):
  - Inspects the original `source` metadata to determine whether the symlink should be created as a directory or file symlink (`symlink_dir` vs. `symlink_file`).
- For other platforms, compilation fails with an explicit `compile_error!`, signaling unsupported environments.
- All variants bubble up filesystem errors as `GitToolingError::Io`.

## Broader Context
- Invoked by ghost commit restoration when git expects symlinks to be recreated during checkout.
- Keeps platform branching isolated so higher-level code can call `create_symlink` without conditional compilation.

## Technical Debt
- None; behavior mirrors git’s symlink expectations across supported platforms.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./ghost_commits.rs.spec.md
