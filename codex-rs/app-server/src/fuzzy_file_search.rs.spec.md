## Overview
`fuzzy_file_search` fans out fuzzy-match queries across multiple project roots and collects scored matches for the VS Code extension. It wraps the `codex-file-search` crate with cancellation support and result normalization for the app server protocol.

## Detailed Behavior
- Constants throttle work:
  - `LIMIT_PER_ROOT` restricts results to at most 50 per root.
  - `MAX_THREADS` caps parallelism, while `COMPUTE_INDICES` controls whether character indices are returned.
- `run_fuzzy_file_search`:
  - Calculates the available parallelism, divides threads among the requested roots (ensuring at least one thread per root), and spawns blocking search jobs via `JoinSet`.
  - Each job invokes `file_search::run` with the cancellation flag, returning either matches or an error tagged with the originating root.
  - Aggregates successes into `FuzzyFileSearchResult`s, synthesizing `file_name` when the path lacks a trailing component.
  - Logs warnings for search errors or task panics.
  - Sorts results with `cmp_by_score_desc_then_path_asc` to prioritize higher scores and keep ordering deterministic.
- Cancellation: if the shared `AtomicBool` is set, the underlying `file_search` implementation stops processing; callers can reuse the same flag for multiple requests.

## Broader Context
- Called by `CodexMessageProcessor::fuzzy_file_search` (`./codex_message_processor.rs.spec.md`), which manages cancellation tokens and routes responses back to the client.
- Shares scoring semantics with other components that reuse `codex-file-search`, ensuring consistent match ordering across the platform.

## Technical Debt
- File-name derivation lives in this adapter (`TODO(shijie)`); ideally the `codex-file-search` crate should emit display names directly.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Move display-name generation into the `codex-file-search` crate.
related_specs:
  - ./codex_message_processor.rs.spec.md
