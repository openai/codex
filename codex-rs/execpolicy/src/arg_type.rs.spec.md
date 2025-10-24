## Overview
`execpolicy::arg_type` defines the runtime interpretation of argument types. `ArgType` provides validation and metadata used when constructing `ValidExec` entries and enforcing filesystem rules.

## Detailed Behavior
- Variants: `Literal`, `OpaqueNonFile`, `ReadableFile`, `WriteableFile`, `PositiveInteger`, `SedCommand`, `Unknown`.
- `validate(&self, value)` implements type-specific checks:
  - `Literal` ensures exact match.
  - File types reject empty paths.
  - `PositiveInteger` enforces numeric > 0.
  - `SedCommand` delegates to `parse_sed_command` for safe range syntax.
  - `OpaqueNonFile`/`Unknown` accept any string.
- `might_write_file()` returns true for `WriteableFile` and `Unknown`, informing `ValidExec::might_write_files`.
- Implements Starlark integration via `StarlarkValue`.

## Broader Context
- `ArgMatcher::arg_type` maps patterns to these types. Validation occurs when building `MatchedArg`/`MatchedOpt`, surfacing errors back to policy authors or runtime checks.
- `ExecvChecker` relies on types to determine which arguments require path whitelisting.
- Context can't yet be determined for richer types (e.g., URLs); new variants will require updating validators.

## Technical Debt
- `Unknown` conflates “unknown but potentially dangerous” with “no validation”; consider more granular markers (e.g., `MaybeFile`) to reduce false positives when checking writable folders.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Explore additional arg types (e.g., directories vs files, enumerations) to tighten policy enforcement without relying on `Unknown`.
related_specs:
  - ./arg_matcher.rs.spec.md
  - ./sed_command.rs.spec.md
  - ./execv_checker.rs.spec.md
