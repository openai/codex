## Overview
`core::truncate` provides utilities for shortening large strings while preserving leading and trailing context. The primary consumer is unified exec and command outputs, which need readable snippets without exceeding storage or token budgets.

## Detailed Behavior
- `truncate_middle(s, max_bytes)`:
  - Returns `(s.to_string(), None)` when the input fits within `max_bytes`.
  - Estimates tokens as `ceil(len/4)` to report truncation to the caller.
  - Handles `max_bytes == 0` by returning a pure marker (`…N tokens truncated…`).
  - Otherwise:
    1. Computes available budget after reserving space for the marker.
    2. Chooses prefix/suffix windows (`pick_prefix_end`, `pick_suffix_start`) favouring newline boundaries to keep output aligned.
    3. Iteratively refines the marker token count (up to four passes) so the marker reflects actual truncated tokens.
    4. Ensures UTF-8 boundaries by backing off to character boundaries when needed.
  - Appends a newline between the marker and suffix to make the output readable.
- Tests cover newline preferences, UTF-8 handling, zero budgets, and deterministic outputs to avoid regressions.

## Broader Context
- Unified exec (`unified_exec/mod.rs`) and tool handlers call this helper before emitting large command transcripts. Token estimates feed telemetry and reviewer context to explain how much data was dropped.
- Context can't yet be determined for multi-marker strategies (e.g., prefix-only or suffix-only truncation); this module currently serves the symmetric use case.

## Technical Debt
- The token estimate assumes 4 bytes per token; exposing tokenizer-aware hooks would improve accuracy for languages with different byte/token ratios.
- Helper functions are private; if other modules need to choose prefix/suffix splits independently, extracting reusable boundary utilities would reduce duplication.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Integrate tokenizer-aware token counting so truncation markers reflect actual model limits instead of byte heuristics.
related_specs:
  - ./unified_exec/mod.rs.spec.md
  - ./tools/handlers/read_file.rs.spec.md
