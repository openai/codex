## Overview
`execpolicy::lib` exposes the building blocks for Codex’s exec policy engine. It re-exports the types that parse Starlark policy files, classify commands, and validate filesystem permissions, and it embeds the default policy used by `codex-exec`.

## Detailed Behavior
- Modules:
  - `arg_matcher`, `arg_resolver`, `arg_type`, `opt`: define the vocabulary for describing positional arguments and options in policy definitions.
  - `policy` / `policy_parser`: parse Starlark policy files (including the embedded `default.policy`) into `Policy` values.
  - `program`, `valid_exec`: perform per-program validation and capture matched command metadata.
  - `exec_call`, `execv_checker`: represent observed commands and enforce filesystem/path constraints.
  - `sed_command`, `arg_matcher::SedCommand`: bespoke validation for safe `sed` invocations.
  - `error`: common error enum returned during validation.
- Public API re-exports `Policy`, `PolicyParser`, `ExecvChecker`, `MatchedExec`, `ValidExec`, and helper types so callers (CLI or core runtime) can load policies and run checks.
- `DEFAULT_POLICY` embeds `default.policy`; `get_default_policy()` parses it using `PolicyParser::new("#default", DEFAULT_POLICY)`.

## Broader Context
- `codex-exec` uses `ExecvChecker` and `get_default_policy` to validate shell commands before execution. The CLI (`execpolicy/src/main.rs`) wraps these exports for manual policy testing.
- Policy parsing relies on Starlark (via `starlark` crate). Updates to policy syntax must keep parser and `default.policy` in sync.
- Context can't yet be determined for per-platform policy tweaks; current implementation assumes a shared policy definition.

## Technical Debt
- Module list is broad; breaking the crate into subpackages (parser vs runtime checker) would make responsibilities clearer.
- `default.policy` is opaque at compile time; adding documentation or tests around the embedded policy would make future updates safer.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Consider splitting parser/runtime layers to simplify re-use in other crates.
    - Add coverage/tests describing the embedded `default.policy` so changes are intentional.
related_specs:
  - ./policy_parser.rs.spec.md
  - ./policy.rs.spec.md
  - ./program.rs.spec.md
  - ./execv_checker.rs.spec.md
