## Overview
`core::command_safety::is_dangerous_command` detects shell commands that likely require elevated scrutiny. It flags Git operations that mutate history or files, recursively deletes (`rm -rf`), and honours shell wrappers so dangerous commands embedded in `bash -lc` sequences are caught.

## Detailed Behavior
- `command_might_be_dangerous` checks `is_dangerous_to_call_with_exec` directly on the command. If the command is `bash -lc`/`zsh -lc`, it parses the script with `parse_shell_lc_plain_commands` and returns `true` when any constituent command is dangerous.
- `is_dangerous_to_call_with_exec`:
  - Treats `git reset`/`git rm` (including paths ending in `/git`) as dangerous.
  - Flags `rm -f` and `rm -rf`.
  - Handles `sudo` by recursively inspecting the underlying command.
- Unit tests cover direct, shell-wrapped, and sudo-wrapped dangerous commands as well as safe alternatives (`git status`).

## Broader Context
- The orchestrator uses this module to decide whether additional approvals or escalated sandbox retries are warranted. Combining it with `is_safe_command` gives a nuanced view of command risk.
- Because the logic is intentionally minimal, any new destructive command families should be added here to keep safeguards effective.

## Technical Debt
- None noted; the heuristics are straightforward and easily extended as new patterns are identified.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./is_safe_command.rs.spec.md
  - ../bash.rs.spec.md
