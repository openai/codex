## Overview
`common::format_env_display` turns an optional environment map and an ordered list of variable names into a human-readable string. The helper is shared by CLI tools that display command previews or diagnostics, ensuring environment data appears consistently across interfaces.

## Detailed Behavior
- Accepts an optional `HashMap<String, String>` containing explicit environment overrides and a slice of variable names that should display as shell-expanded placeholders.
- When a map is present, clones its entries into a sortable vector, sorts lexicographically by key, and formats each pair as `"KEY=value"`. Sorting stabilizes the output regardless of the map’s insertion order.
- For every name in `env_vars`, appends a placeholder formatted as `"VAR=$VAR"`, preserving the caller-provided order.
- Returns `"-"` when both sources are empty; otherwise joins all pieces with `", "` to produce a single line that is easy to scan in logs and tables.
- Keeps allocation predictable by accumulating strings in a `Vec<String>` before joining, which allows consumers to display or log the result without additional formatting.

## Broader Context
- Used by command renderers in the CLI and TUI to indicate which environment variables will be present during command execution. Aligns with the representation expected by shell-savvy users.
- The helper intentionally does not mask secrets; higher-level callers must redact sensitive values before passing them in if necessary.
- Context can't yet be determined for whether non-UTF-8 environment values need special handling; revisit if such cases surface in specs for command execution modules.

## Technical Debt
- None observed; the function is deterministic, covered by unit tests, and free of outstanding TODOs.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./config_override.rs.spec.md
