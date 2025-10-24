## Overview
`core::rollout::tests` exercises the rollout listing pipeline end-to-end. The module synthesizes on-disk session trees to validate pagination, filtering, tail sampling, and cursor semantics used by the runtime.

## Detailed Behavior
- Helpers like `write_session_file` materialize JSONL rollouts with deterministic timestamps and UUIDs so tests can assert exact paths and payloads.
- Coverage highlights:
  - `test_list_conversations_latest_first`, `test_pagination_cursor`, and `test_stable_ordering_same_second_pagination` confirm descending order, cursor continuity, and stable ordering within identical timestamp buckets.
  - `test_get_conversation_contents` validates the `get_conversation` helper and page equality for single-item listings.
  - Tail behavior is thoroughly exercised: `test_tail_includes_last_response_items`, `test_tail_handles_short_sessions`, and `test_tail_skips_trailing_non_responses` ensure the tail sampler returns only response items and derives `updated_at` correctly.
  - `test_source_filter_excludes_non_matching_sessions` verifies source filtering against `INTERACTIVE_SESSION_SOURCES`.
- The suite leans on `tokio::test` to cover async traversal, ensuring the real code paths (directory reads, JSON parsing) behave under concurrency.

## Broader Context
- These tests guard the contract relied upon by CLI/TUI commands that surface rollout history; failing assertions typically mean user-facing regressions in pagination or filtering.
- Context can't yet be determined for archived session coverage; tests operate exclusively on the active `sessions/` tree.

## Technical Debt
- Synthetic JSONL entries omit reasoning/tool-call variants, leaving gaps in coverage for complex `RolloutItem` shapes.
- Tests write to the filesystem with blocking APIs (e.g., `File::create`); migrating to async helpers would better mirror production paths.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Expand fixtures to include reasoning, tool call, and non-message events to guarantee filtering/pagination works across all persisted variants.
related_specs:
  - ./mod.rs.spec.md
  - ./list.rs.spec.md
  - ./policy.rs.spec.md
