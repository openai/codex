## Overview
`core::rollout::policy` encapsulates the filtering rules that decide which protocol items are persisted to rollout JSONL files. It converts the high-volume event stream emitted during a session into a compact record of user/agent interactions that matter for replay and analysis.

## Detailed Behavior
- `is_persisted_response_item` branches on `RolloutItem` variants:
  - Response items and function/tool call artifacts are kept (`should_persist_response_item`).
  - Event messages are forwarded to `should_persist_event_msg`.
  - Structural markers (`Compacted`, `TurnContext`, `SessionMeta`) are always persisted to preserve replay correctness.
- `should_persist_response_item` whitelists conversational content (messages, reasoning, tool calls/results, web search calls) and drops `ResponseItem::Other` to avoid vendor-specific noise.
- `should_persist_event_msg` retains user/agent messages, reasoning updates, token counts, review-mode transitions, and aborts while discarding low-level progress events, tool begin/end notifications, and streaming deltas that would bloat rollouts without adding replay value.

## Broader Context
- `RolloutRecorder::record_items` uses these helpers to filter every batch before enqueueing writes. The same rules apply in `rollout_writer`, ensuring late-stage filters stay consistent.
- By centralizing the policy, CLI/TUI tooling that reads rollouts can rely on a predictable schema: user inputs, agent outputs, structural metadata, and review transitions.

## Technical Debt
- The allowlist mirrors protocol enums manually; when new `ResponseItem` or `EventMsg` variants are introduced there is no compile-time enforcement to update the rollout policy.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Replace the manual `match` allowlists with pattern groups or derive-based metadata so new protocol variants cannot bypass rollout persistence review.
related_specs:
  - ./mod.rs.spec.md
  - ./recorder.rs.spec.md
  - ../protocol/src/protocol.rs.spec.md
