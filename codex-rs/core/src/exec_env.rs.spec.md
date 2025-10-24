## Overview
`core::exec_env` builds sanitized environment maps for shell executions. It applies the rules encoded in `ShellEnvironmentPolicy`, ensuring spawned commands inherit only the intended environment variables.

## Detailed Behavior
- `create_env` fetches the parent process environment and delegates to `populate_env`, which applies policy steps in order:
  1. **Inheritance**: depending on `ShellEnvironmentPolicyInherit`, start with all variables, a curated core subset (`HOME`, `PATH`, etc.), or none.
  2. **Default excludes**: unless `ignore_default_excludes` is set, remove variables whose names match case-insensitive patterns `*KEY*`, `*SECRET*`, or `*TOKEN*`.
  3. **Custom excludes**: remove variables matching the policy’s `exclude` patterns.
  4. **Overrides (`set`)**: inject or replace specific key/value pairs supplied by the policy.
  5. **Include-only filter**: if specified, retain only variables matching the given patterns.
- Environment patterns use `EnvironmentVariablePattern` (glob-style matching). Internal helper `matches_any` checks a name against a list of patterns.
- Unit tests cover combinations of inheritance modes, default exclude behaviour, include-only filtering, and overrides to ensure edge cases (case-insensitive patterns, zero inheritances) behave as documented.

## Broader Context
- Shell and apply_patch handlers use `create_env` when constructing `ExecParams`, guaranteeing consistent environment sanitisation across tool executions.
- Policies are derived from user configuration (`codex-common`) and may be updated per turn, so the helper must remain deterministic and order-stable.
- Context can't yet be determined for platform-specific variables (e.g., Windows `SystemRoot`); current logic treats all keys uniformly.

## Technical Debt
- None noted; behaviour mirrors the policy documentation and is extensively tested.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./exec.rs.spec.md
  - ./tools/handlers/shell.rs.spec.md
  - ../config_types.rs.spec.md
