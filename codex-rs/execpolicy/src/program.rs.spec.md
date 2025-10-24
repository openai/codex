## Overview
`execpolicy::program` defines `ProgramSpec`, the per-program rule set produced by policies. It validates command-line invocations against allowed options, argument patterns, and examples, and reports whether a command is allowed, forbidden, or malformed.

## Detailed Behavior
- `ProgramSpec` fields include:
  - Program name and canonical system paths.
  - Option handling (`option_bundling`, `combined_format`, `allowed_options`, `required_options`).
  - Argument patterns (`arg_patterns`: list of `ArgMatcher`s).
  - Optional `forbidden` reason to force-match commands into forbidden status.
  - Positive/negative example lists for policy self-checking.
- `ProgramSpec::check(exec_call)`:
  - Iterates arguments in order, tracking `expecting_option_value` for options that take values.
  - Errors when encountering unknown options, missing option values, or unsupported `--` sequences.
  - Delegates positional argument resolution to `resolve_observed_args_with_patterns`, yielding typed `MatchedArg`s.
  - Verifies required options were provided (`missing_required_options` error otherwise).
  - Builds `ValidExec` containing matched flags, options, arguments, and system path hints.
  - Applies the `forbidden` reason (if present) to return `MatchedExec::Forbidden`, otherwise `MatchedExec::Match`.
- Example verification:
  - `verify_should_match_list` iterates positive examples, re-running `check` and capturing failures as `PositiveExampleFailedCheck`.
  - `verify_should_not_match_list` does the opposite, ensuring negative examples do not produce matches.
- `MatchedExec`/`Forbidden` enums encapsulate match outcome and reason (program-level, argument-level, or exec-level).

## Broader Context
- `Policy::check` relies on `ProgramSpec::check` for actual argument analysis once high-level filters pass. The resulting `ValidExec` flows into `ExecvChecker` for filesystem safety checks.
- `ArgMatcher`, `ArgType`, `Opt`, and `arg_resolver` collaborate here; any change to matchers or option semantics must be reflected across these modules.
- Context can't yet be determined for bundled/combined option formats; TODO comments signal future enhancements.

## Technical Debt
- Option handling is strict (no `--option=value` support yet); extending to combined formats will require revisiting the parsing logic.
- The check loop is dense; refactoring into smaller helpers (option parsing vs positional matching) would improve readability.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add support for `--option=value` and `--` separators, as hinted by TODOs/errors, to cover common CLI patterns.
    - Break out option parsing vs positional argument processing to simplify future modifications.
related_specs:
  - ./arg_resolver.rs.spec.md
  - ./arg_matcher.rs.spec.md
  - ./valid_exec.rs.spec.md
  - ./policy.rs.spec.md
