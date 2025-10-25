## Overview
`errors.rs` defines the `GitToolingError` enum used throughout the git-tooling crate to report subprocess failures, invalid paths, and IO issues with actionable context.

## Detailed Behavior
- Variants include:
  - `GitCommand` capturing the full command string, exit status, and stderr when `git` returns a non-zero status.
  - `GitOutputUtf8` wrapping `FromUtf8Error` when git stdout cannot be decoded.
  - Repository validation errors (`NotAGitRepository`, `NonRelativePath`, `PathEscapesRepository`).
  - Wrapper conversions for `StripPrefixError`, `walkdir::Error`, and generic `std::io::Error`.
- Derives `thiserror::Error` for ergonomic display and `Debug` formatting, which downstream code uses to surface meaningful messages to users.

## Broader Context
- Returned by helpers in `operations.rs` and `ghost_commits.rs`, enabling core orchestration layers to distinguish user misconfiguration from transient git errors.
- Propagated to Codex UX surfaces (CLI/TUI) so detailed diagnostics (status code, stderr) appear in error toasts or logs.

## Technical Debt
- None; the enum already covers the failure modes exercised across the crate.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./operations.rs.spec.md
  - ./ghost_commits.rs.spec.md
