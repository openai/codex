## Overview
`execpolicy::policy` holds the compiled policy representation. `Policy` maps program names to `ProgramSpec` entries, tracks forbidden program regexes, and enforces forbidden argument substrings.

## Detailed Behavior
- Construction:
  - `Policy::new` stores a `MultiMap<String, ProgramSpec>`, a vector of `ForbiddenProgramRegex`, and compiles forbidden substrings (if any) into a single OR-regex.
- `check(exec_call: &ExecCall)`:
  - Inspects program name against forbidden regexes; returns `MatchedExec::Forbidden` with reason if matched.
  - Tests arguments against the forbidden substring regex; any match produces a forbidden result.
  - Looks up `ProgramSpec` entries for the program. If none match successfully, returns the last error (defaulting to `Error::NoSpecForProgram`).
  - On success, returns `MatchedExec::Match` or `MatchedExec::Forbidden` (if the spec itself marks the exec as forbidden).
- `check_each_good_list_individually` / `check_each_bad_list_individually` iterate program specs to validate positive/negative examples defined in the policy, returning violations for diagnostics or tests.

## Broader Context
- `ExecvChecker` relies on `Policy::check` to classify commands before applying filesystem permissions.
- Policy validation ensures approval rules stay congruent across CLI (`execpolicy` tool), the embedded default policy, and Codex runtime enforcement.
- Context can't yet be determined for policy hot-reloading; current usage assumes policies are parsed once per invocation.

## Technical Debt
- `check` keeps only the last error when multiple specs exist; augmenting diagnostics (e.g., aggregate reasons) would help policy authors debug mismatches.
- Regex compilation happens at policy creation; invalid patterns produce immediate errors but there’s no caching for repeated construction.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Enhance error reporting when multiple specs fail to match a program (include reasons from each spec).
related_specs:
  - ./policy_parser.rs.spec.md
  - ./program.rs.spec.md
  - ./execv_checker.rs.spec.md
