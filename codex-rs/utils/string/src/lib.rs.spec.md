## Overview
`codex_utils_string::lib` implements prefix and suffix truncation helpers that maintain UTF-8 integrity by cutting only at code point boundaries.

## Detailed Behavior
- `take_bytes_at_char_boundary(s, maxb)` returns the longest prefix of `s` whose byte length does not exceed `maxb`. It walks `char_indices`, tracking the last complete code point whose end offset falls within the budget.
- `take_last_bytes_at_char_boundary(s, maxb)` returns a suffix limited to `maxb` bytes. It iterates backward over characters, accumulating byte lengths until the budget would be exceeded, then returns the remaining tail.
- Both functions short-circuit when the input already fits the budget, avoiding unnecessary iteration for common short strings.

## Broader Context
- Used throughout `codex-core` tool handlers to produce truncated previews (e.g., file contents, directory listings, telemetry payloads) without corrupting UTF-8 data returned to models or users.
- Truncation helpers support higher-level formatting utilities in `core/src/tools/mod.rs`, `core/src/tools/context.rs`, and `core/src/tools/handlers/{list_dir,read_file}.rs`.

## Technical Debt
- None identified; algorithms are straightforward and derive correctness from `char_indices`. Additional unit tests could be added downstream if new edge cases appear.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../../core/src/tools/mod.rs.spec.md
  - ../../core/src/tools/context.rs.spec.md
  - ../../core/src/tools/handlers/list_dir.rs.spec.md
  - ../../core/src/tools/handlers/read_file.rs.spec.md
