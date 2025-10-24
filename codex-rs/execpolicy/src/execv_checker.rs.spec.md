## Overview
`execpolicy::execv_checker` verifies executable commands against filesystem constraints after policy matching. It ensures readable/writeable arguments stay within approved directories and selects the executable path to spawn.

## Detailed Behavior
- `ExecvChecker::new` stores a compiled `Policy`.
- `match(&ExecCall)` reuses `Policy::check`, returning `MatchedExec` for inspection when desired.
- `check(valid_exec, cwd, readable_folders, writeable_folders)`:
  - Iterates over all typed arguments (`ValidExec::args` and `opts`), validating filesystem expectations:
    - `ReadableFile` must reside under one of `readable_folders`.
    - `WriteableFile` under `writeable_folders`.
    - Paths are absolutized using `path_absolutize`, respecting an optional `cwd`.
    - Errors (`ReadablePathNotInReadableFolders`, etc.) bubble back if constraints fail.
  - Chooses the executable path:
    - Defaults to `valid_exec.program`.
    - If any `system_path` entry points to a real executable (checked via metadata/permission bits), uses the first match instead for added safety.
- Helper macros and functions:
  - `check_file_in_folders!` enforces directory containment.
  - `ensure_absolute_path` canonicalizes relative paths (erroring when `cwd` is missing).
  - `is_executable_file` inspects filesystem metadata, checking execute bits on Unix (TODO for Windows).

## Broader Context
- Used by Codex core before spawning commands to respect sandbox policies. The CLI `execpolicy` tool leverages the same logic when `--require-safe` is set.
- Works in concert with policy definitions; `ArgType::ReadableFile` / `WriteableFile` must be applied appropriately to get the desired enforcement.
- Context can't yet be determined for network-related checks; current scope is filesystem-focused.

## Technical Debt
- Windows executable detection is incomplete (`TODO`); once Windows support is required, rely on `PATHEXT` or similar.
- Error messages reflect raw paths; including original argument positions could aid debugging.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Implement Windows-aware executable detection and normalization.
    - Enrich filesystem error messages with argument indices to guide policy authors.
related_specs:
  - ./policy.rs.spec.md
  - ./valid_exec.rs.spec.md
