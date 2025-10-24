## Overview
`execpolicy::opt` models command-line options in policies. `Opt` describes allowed flags (`--flag`) and value-bearing options (`--opt <value>`), including whether they are required.

## Detailed Behavior
- `Opt` fields: option string, metadata (`OptMeta`), and required flag.
- `OptMeta` variants:
  - `Flag`: no value expected.
  - `Value(ArgType)`: expects one argument validated against the provided type.
- Helper methods:
  - `Opt::new` constructs options.
  - `Opt::name` returns the option string (used for deduplication and matching).
- Starlark integration:
  - Implements `StarlarkValue`, `AllocValue`, and `UnpackValue` so policies can call `{ opt(...), flag(...) }`.
- Required options feed into `ProgramSpec::required_options`; missing ones trigger `Error::MissingRequiredOptions`.

## Broader Context
- Option definitions flow from policies into `ProgramSpec::check`, which distinguishes flags (recorded as `MatchedFlag`) from options requiring subsequent values (`MatchedOpt`).
- Combined format (`--opt=value`) is currently unsupported (TODO elsewhere). Policies must define options accordingly.

## Technical Debt
- Policy DSL can’t express aliases or grouped options; supporting those would require richer metadata.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Extend metadata to support option aliases or combined formats once parsing logic is enhanced.
related_specs:
  - ./program.rs.spec.md
  - ./arg_type.rs.spec.md
