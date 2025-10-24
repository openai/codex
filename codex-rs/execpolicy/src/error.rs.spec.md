## Overview
`execpolicy::error` defines the unified error type returned throughout the policy engine. It captures parsing failures, matcher mismatches, filesystem violations, and internal invariants, allowing callers to present actionable diagnostics.

## Detailed Behavior
- `Error` (serde-serializable enum) includes variants for:
  - Policy mismatches: unknown program, missing option values, unexpected arguments, unsupported `--`, multiple varargs, insufficient args, etc.
  - Validation failures: literal mismatch, invalid positive integers, unsafe `sed` commands, missing required options.
  - Filesystem enforcement: readable/writeable path outside allowed folders, relative path without cwd, canonicalization errors.
  - Internal errors (prefix overlaps suffix, invariant violation) indicating bugs in matcher logic.
- Each variant stores contextual data (program name, offending argument, range indices) to aid debugging. Serde annotations (`serde_as`, `DisplayFromStr`) ensure JSON output remains readable.
- `Result<T>` alias centralizes error handling across modules.

## Broader Context
- The CLI (`execpolicy`) serializes these errors in JSON responses (e.g., `unverified` results). Codex core can surface them in logs when commands fail policy checks.
- Adding new validation logic should introduce matching error variants to keep diagnostics clear.

## Technical Debt
- Error messages are machine-oriented; providing human-friendly strings or helper formatting functions would improve UX in CLI outputs.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add descriptive display implementations to produce reader-friendly messages without requiring custom handling at call sites.
related_specs:
  - ./arg_resolver.rs.spec.md
  - ./program.rs.spec.md
  - ./execv_checker.rs.spec.md
