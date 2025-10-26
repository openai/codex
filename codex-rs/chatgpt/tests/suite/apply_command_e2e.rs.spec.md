## Overview
`apply_command_e2e` validates the chatgpt crate’s ability to apply hosted diff tasks into a local Git repository, covering both success and merge-conflict scenarios.

## Detailed Behavior
- Helper `create_temp_git_repo` initializes a clean repository with deterministic Git config, ensuring tests run hermetically.
- `mock_get_task_with_fixture` loads `task_turn_fixture.json` to simulate a ChatGPT diff response.
- `test_apply_command_creates_fibonacci_file` applies the fixture diff via `apply_diff_from_task` and asserts that `scripts/fibonacci.js` exists with the expected contents and line count.
- `test_apply_command_with_merge_conflicts` seeds a conflicting file, applies the diff, expects an error, and confirms merge-conflict markers remain in the file.

## Broader Context
- Provides integration coverage for `apply_command` logic documented in Phase 4, confirming Git operations behave correctly against real repositories.

## Technical Debt
- Tests rely on the system `git` binary; adding guards for environments lacking Git could improve portability.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add feature detection or skips for environments without Git to prevent spurious failures.
related_specs:
  - ../../mod.spec.md
  - ../../src/apply_command.rs.spec.md
