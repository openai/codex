## Overview
`execpolicy::sed_command` validates `sed` invocations referenced by policies. It currently supports a conservative subset of expressions to ensure automated edits remain safe.

## Detailed Behavior
- `parse_sed_command(s: &str)` accepts only range-print commands matching the pattern `<start>,<end>p` where `start` and `end` are positive integers.
- Trailing `p` is mandatory; absence or additional syntax results in `Error::SedCommandNotProvablySafe`.
- Successful parsing returns `Ok(())`; errors wrap the offending command string for diagnostic output.

## Broader Context
- `ArgType::SedCommand` relies on this function during argument validation. Policies that use `ARG_SED_COMMAND` guarantee that only simple, range-based sed prints are allowed.
- Expanding sed support requires careful hardening to avoid arbitrary command execution (e.g., no `s///` or shell escapes).

## Technical Debt
- Limited command coverage; a richer parser (supporting multiple operations or flags) would be valuable if policies need them, but must remain security-conscious.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Extend the safe subset only after thorough analysis; consider integrating a proper sed parser to expand functionality without compromising safety.
related_specs:
  - ./arg_type.rs.spec.md
  - ./program.rs.spec.md
