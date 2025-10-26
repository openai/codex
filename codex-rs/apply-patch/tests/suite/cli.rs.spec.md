## Overview
`cli` verifies the standalone `apply_patch` binary can apply patches from both command-line arguments and stdin, covering add and update operations.

## Detailed Behavior
- `test_apply_patch_cli_add_and_update` creates a temp directory, runs `apply_patch <patch>` to add a file, then runs a second patch to update it, asserting stdout displays the expected status codes (`A`, `M`) and the file contents change accordingly.
- `test_apply_patch_cli_stdin_add_and_update` repeats the workflow but pipes patches via stdin to ensure standard input is accepted.

## Broader Context
- Confirms the CLI remains compatible with tooling expectations described in Phase 3, independent of the Codex multitool.

## Technical Debt
- Tests do not cover delete patches or failure scenarios; add coverage if regressions arise.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../../src/lib.rs.spec.md
