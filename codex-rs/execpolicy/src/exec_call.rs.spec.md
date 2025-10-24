## Overview
`execpolicy::exec_call` provides a minimal representation of a command invocation (`ExecCall`). Policies and checkers operate on this struct when classifying commands or reporting errors.

## Detailed Behavior
- `ExecCall` stores the program string and argument vector.
- `ExecCall::new(program, args)` builds an instance from `&str` slices.
- Implements `Display` by joining program and args with spaces (used in diagnostics).
- Derives `Serialize` for easy reporting (e.g., JSON output in the CLI).

## Broader Context
- `Policy::check` and `ExecvChecker::match` accept `ExecCall`. CLI tooling converts user-provided commands (raw or JSON) into this struct before evaluation.
- Context can't yet be determined for metadata (e.g., environment); additional fields would require updating policy logic.

## Technical Debt
- Simple struct; no major concerns.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./policy.rs.spec.md
  - ./execv_checker.rs.spec.md
