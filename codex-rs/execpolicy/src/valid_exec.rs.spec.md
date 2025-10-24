## Overview
`execpolicy::valid_exec` captures the normalized result of a successful policy match. `ValidExec` stores the program name, matched flags/options/args (with validated types), and optional system-path overrides for safer execution.

## Detailed Behavior
- `ValidExec` fields:
  - `program`: canonical name from the policy.
  - `flags`: `MatchedFlag` entries (no value).
  - `opts`: `MatchedOpt` (name, value, `ArgType`).
  - `args`: `MatchedArg` (index, type, value).
  - `system_path`: prioritized fallback paths to prefer over the original program (e.g., `/bin/ls`).
- Helpers:
  - `ValidExec::new` builds a minimal instance (used in tests).
  - `might_write_files()` inspects matched options/args to determine if the command could create/modify files (leveraging `ArgType::might_write_file`).
- `MatchedArg::new` and `MatchedOpt::new` validate values against the specified `ArgType`, returning errors when expectations fail.
- `MatchedFlag::new` stores flag names for reporting.

## Broader Context
- `ExecvChecker` uses `ValidExec` to enforce filesystem permissions and locate executable paths. The CLI (`execpolicy`) returns `ValidExec` instances in JSON output (`safe`/`match` results).
- `ProgramSpec::check` populates these structures; downstream tooling relies on the metadata to make execution decisions.

## Technical Debt
- `system_path` is a simple list; future enhancements might track why a path was chosen (e.g., policy vs environment).

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./program.rs.spec.md
  - ./execv_checker.rs.spec.md
