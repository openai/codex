## Overview
`execpolicy::arg_matcher` defines `ArgMatcher`, the Starlark-exposed pattern language for positional arguments in exec policies. Each matcher encapsulates cardinality rules and the resulting `ArgType` used for validation.

## Detailed Behavior
- `ArgMatcher` variants include:
  - `Literal(String)`: exact string match.
  - `OpaqueNonFile`: non-path values.
  - `ReadableFile`, `WriteableFile`, `ReadableFiles`, `ReadableFilesOrCwd`.
  - `PositiveInteger`, `SedCommand`.
  - `UnverifiedVarargs`: catch-all for arbitrary trailing arguments.
- `cardinality()` returns `ArgMatcherCardinality` (one, at least one, zero-or-more). Helper `is_exact` informs argument partitioning.
- `arg_type()` maps each matcher to an `ArgType` (handled by validation).
- Implements Starlark traits (`AllocValue`, `StarlarkValue`, `UnpackValue`) so matchers can be used in policy files (globals like `ARG_RFILE`).

## Broader Context
- `ArgMatcher` drives positional argument validation in `ProgramSpec::check` via `arg_resolver`. The Starlark DSL uses these symbols to describe expected CLI structure.
- New matchers must update both Rust and policy DSL; consider schema compatibility when extending.

## Technical Debt
- Some matchers (e.g., `ReadableFiles`, `ReadableFilesOrCwd`) rely on subsequent logic to interpret aggregated values; adding richer structured matchers could make downstream logic simpler.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Consider adding structured matchers (e.g., enumerations, regex) if policies require more expressiveness.
related_specs:
  - ./arg_type.rs.spec.md
  - ./arg_resolver.rs.spec.md
