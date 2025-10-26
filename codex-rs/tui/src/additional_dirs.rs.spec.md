## Overview
`additional_dirs` handles messaging for the `--add-dir` CLI option, warning users when their sandbox policy prevents additional writable directories from taking effect.

## Detailed Behavior
- `add_dir_warning_message(additional_dirs, sandbox_policy)` returns `None` when:
  - No extra directories are configured, or
  - The sandbox policy permits writes (`WorkspaceWrite`, `DangerFullAccess`).
- When the policy is `ReadOnly`, it calls `format_warning` to generate a human-friendly explanation listing the ignored paths and suggesting compatible sandbox modes.
- Helper `format_warning` joins directory paths with commas, preserving original formatting via `to_string_lossy`.
- Unit tests verify each branch, ensuring CLI feedback stays consistent when new sandbox policies are added.

## Broader Context
- Invoked by the TUI CLI and other frontends before applying sandbox overrides, ensuring users understand why extra writable roots are ignored in read-only configurations.

## Technical Debt
- None; logic is intentionally small and easily extensible if new policies appear.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./cli.rs.spec.md
  - ./tui.rs.spec.md
