## Overview
`execpolicy::policy_parser` loads Starlark policies and produces `Policy` structures. It registers built-in functions (e.g., `define_program`, `opt`, `flag`) that let policy authors describe allowed programs, argument patterns, and forbidden substrings or regexes.

## Detailed Behavior
- `PolicyParser::parse`:
  - Configures the Starlark `Dialect` (extended mode with f-strings).
  - Builds globals with `policy_builtins` and Typing extensions.
  - Seeds the module with preallocated `ArgMatcher` constants (e.g., `ARG_RFILE`, `ARG_POS_INT`).
  - Evaluates the policy source while storing definitions in a `PolicyBuilder`.
  - Returns `Policy::new` constructed from accumulated program specs, forbidden regexes, and substrings; Starlark errors are wrapped for the caller.
- `PolicyBuilder` (ProvidesStaticType):
  - Holds `MultiMap<String, ProgramSpec>` for per-program definitions.
  - Collects `ForbiddenProgramRegex` instances (regex + reason) and forbidden substrings (later combined into a single regex).
  - `build` finalizes the `Policy`.
- Builtins:
  - `define_program` registers `ProgramSpec` with options, argument patterns (`ArgMatcher`), forbidden reasons, and positive/negative example lists.
  - `forbid_substrings` and `forbid_program_regex` populate global restrictions.
  - `opt` / `flag` create `Opt` descriptors (value-bearing options vs flags).
- `ForbiddenProgramRegex` wraps a compiled regex and reason, used by `Policy::check`.

## Broader Context
- Policies drive exec approval in Codex core (and within `codex-exec`). The parser is also exercised by the `execpolicy` CLI for policy testing.
- `ArgMatcher`/`Opt` types are Starlark values exposed to policy authors; maintaining parity between Starlark DSL and Rust types is essential when adding new matchers.
- Context can't yet be determined for policy versioning; DV-specific or per-environment extensions may require namespacing in the future.

## Technical Debt
- Error reporting bubbles Starlark line/column but lacks richer diagnostics for policy authors; adding context (e.g., program name) would assist debugging.
- Regex compilation happens on every parse; caching may be worthwhile if large policies are parsed repeatedly.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Improve policy error messages (attach program context and Starlark stack traces) to ease authoring.
    - Cache or share compiled regexes across parses when policies are reloaded frequently.
related_specs:
  - ./policy.rs.spec.md
  - ./program.rs.spec.md
  - ./arg_matcher.rs.spec.md
