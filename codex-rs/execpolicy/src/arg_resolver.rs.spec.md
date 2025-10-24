## Overview
`execpolicy::arg_resolver` aligns observed positional arguments with policy patterns (`ArgMatcher`). It partitions patterns into prefix/vararg/suffix segments, validates counts, and produces typed `MatchedArg`s consumed by `ProgramSpec::check`.

## Detailed Behavior
- `PositionalArg` records the original index and string value for each argument.
- `resolve_observed_args_with_patterns`:
  - Uses `partition_args` to categorize `arg_patterns` into exact-cardinality prefixes/suffixes and an optional vararg matcher (`ArgMatcherCardinality`).
  - Matches prefix patterns first, enforcing exact counts and converting each value into `MatchedArg` using `ArgMatcher::arg_type().validate`.
  - Determines how many trailing args belong to suffix patterns; ensures prefix and suffix ranges do not overlap and that enough args are available.
  - Vararg handling:
    - `ZeroOrMore` accepts any remaining args (possibly zero).
    - `AtLeastOne` requires at least one arg; otherwise returns `Error::VarargMatcherDidNotMatchAnything`.
    - `One` is considered invalid and triggers an internal invariant error if encountered.
  - After suffix matching, ensures no unmatched arguments remain; extra args trigger `Error::UnexpectedArguments`.
- `partition_args` enforces a single vararg matcher; encountering more yields `Error::MultipleVarargPatterns`.
- `get_range_checked` guards slice accesses, returning range errors when bounds are invalid.

## Broader Context
- This resolver underpins `ProgramSpec::check` and thus the entire validation pipeline. Any new `ArgMatcher` variants must provide sensible cardinalities to integrate here.
- Errors produced here (e.g., `NotEnoughArgs`, `PrefixOverlapsSuffix`) bubble up to policy authors via CLI/JSON responses, helping them adjust argument patterns.
- Context can't yet be determined for advanced matching (e.g., regex patterns within literals); current logic assumes simple cardinals.

## Technical Debt
- Algorithm is “naive” (as noted in comments); supporting interleaved or more complex pattern structures may require a richer matcher or backtracking engine.
- Error messages could include argument values/pattern indexes to aid debugging.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Improve diagnostic detail (include pattern indices and argument excerpts) when matches fail.
    - Explore a more flexible matching engine if policies require multiple vararg sections or optional prefixes.
related_specs:
  - ./arg_matcher.rs.spec.md
  - ./program.rs.spec.md
