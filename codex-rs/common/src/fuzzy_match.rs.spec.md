## Overview
`common::fuzzy_match` implements a Unicode-aware subsequence matcher used by Codex UIs to filter lists as users type. It exposes `fuzzy_match`, which returns the matched indices and a score, and `fuzzy_indices`, a convenience wrapper that only returns the indices.

## Detailed Behavior
- Treats an empty needle as a match with no indices and assigns `i32::MAX` as the score, allowing callers to treat it as the weakest match while still including the item.
- Builds a lowercased version of the haystack by iterating characters and expanding case-folded variants (e.g., `İ → i̇`), simultaneously recording the original character index for each lowered char. This mapping allows index results to point back into the original string even when casing expands to multiple characters.
- Lowercases the needle and walks it against the lowered haystack, scanning forward until each character is found. If any character is missing, the function returns `None`.
- Collects the original indices corresponding to each match and computes a score based on the window size that spans the matches. Prefers contiguous matches (smaller window) and subtracts an additional 100 points when the first match occurs at position 0 to reward prefix hits.
- Deduplicates and sorts indices before returning them so consumers can highlight the correct characters without worrying about multi-codepoint expansions.
- `fuzzy_indices` reuses `fuzzy_match` and strips the score, maintaining the same deduplication logic.
- Extensive unit tests cover ASCII and Unicode behaviors, including dotted İ, German ß, contiguous and non-contiguous matches, and the prefix bonus.

## Broader Context
- Shared by the TUI selection widgets and any other feature that presents incremental search. Consumers rely on the scoring heuristic to sort results, so changes to scoring should be coordinated across those call sites.
- The algorithm provides a simple subsequence matcher rather than a full fuzzy system; if future requirements demand token weighting or typo tolerance, a more sophisticated implementation may replace it.
- Context can't yet be determined for internationalization needs such as locale-specific casing rules; revisit if UI specs surface those constraints.

## Technical Debt
- None observed; the matcher is self-contained, well-tested, and the scoring heuristic is documented in tests.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
