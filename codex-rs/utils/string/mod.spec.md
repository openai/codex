## Overview
`codex-utils-string` provides byte-budget helpers that respect UTF-8 code point boundaries, allowing callers to truncate or window strings without producing invalid sequences.

## Detailed Behavior
- Re-exports `take_bytes_at_char_boundary` for prefix truncation and `take_last_bytes_at_char_boundary` for suffix truncation, both implemented in `src/lib.rs`.
- Keeps the crate lightweight (no additional dependencies) so hot-path callers in `core::tools` can depend on it without adding compile or runtime overhead.

## Broader Context
- Tool handlers in `codex-core` use these helpers when formatting telemetry, tool previews, and directory listings, ensuring rendered UTF-8 stays valid after truncation.
- Context can't yet be determined for potential future consumers outside the `core` crate; revisit after additional audits.

## Technical Debt
- None identified; functions are pure and well-covered by downstream usage.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
