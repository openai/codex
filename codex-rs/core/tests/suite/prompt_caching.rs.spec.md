## Overview
Integration tests that exercise Codex’s prompt caching logic. They spin up mock OpenAI endpoints and verify that instructions, tool lists, and the `<environment_context>` payload stay consistent across turns, only changing when caller-provided context diverges. The suite guards against regressions in how we compose cache keys, merge per-turn overrides, and decide when to resend environment metadata.

## Detailed Behavior
- **Tool/instruction caching**  
  `codex_mini_latest_tools` and `prompt_tools_are_consistent_across_requests` ensure consecutive turns reuse the same instruction prefix and tool list for a given model, asserting both cache key reuse and tool ordering per model family.
- **Prefix stability with overrides**  
  `prefixes_context_and_instructions_once_and_consistently_across_requests`, `overrides_turn_context_but_keeps_cached_prefix_and_key_constant`, and `per_turn_overrides_keep_cached_prefix_and_key_constant` confirm that temporary overrides (via `OverrideTurnContext` or per-turn `UserTurn` fields) change only the trailing environment/user messages while the cached prefix (instructions + prepended context) and `prompt_cache_key` remain stable.
- **Environment context emission**  
  `send_user_turn_with_no_changes_does_not_send_environment_context` verifies that unchanged turns reuse the previous environment context without resending it. `send_user_turn_with_changes_sends_environment_context` demonstrates that switching approval policy, sandbox, or model forces a fresh `<environment_context>` block with only the modified fields. Helper `default_env_context_str` provides the canonical XML snippet; tests compare concrete JSON payloads to catch formatting drift.
- The suite relies on real SSE fixtures (`completed_template.json`) and `core_test_support` harnesses to emulate backend behaviour, so the tests cover full request bodies, not just high-level flags.

## Broader Context
- These tests backstop `core/src/environment_context.rs` and `core/src/prompt_cache.rs`, ensuring downstream services receive consistent context. They also interact with `ConversationManager`, `CodexAuth`, and feature toggles (e.g., ApplyPatch).
- Failures here would manifest as redundant prompt traffic (cache misses) or missing sandbox metadata in API requests.

## Technical Debt
- Tests currently assume network availability (guarded by `skip_if_no_network!`); local runs without network still incur harness setup overhead.
- Mock assertions match exact JSON payloads; minor structural changes require updating several expected snapshots. Adding helper builders for expected payloads could reduce churn.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Introduce shared helpers for expected request payloads to minimise brittle inline JSON comparisons when environment context formatting evolves.
related_specs:
  - ../../src/environment_context.rs.spec.md
  - ../../src/codex.rs.spec.md
