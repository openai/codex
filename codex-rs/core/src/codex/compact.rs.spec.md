## Overview
`core::codex::compact` handles automatic and user-triggered conversation compaction. When a session approaches the model’s context limit, these helpers summarize prior turns, persist rollout records, and refresh the in-memory history so subsequent prompts fit within the window. The module also exposes utility functions for extracting text from content items.

## Detailed Behavior
- `run_inline_auto_compact_task` issues a predefined prompt (`SUMMARIZATION_PROMPT`) to the model, while `run_compact_task` accepts arbitrary `UserInput` provided by upstream automation. Both delegate to `run_compact_task_inner`.
- `run_compact_task_inner` builds an initial `Prompt` from current history (including the new compacting instruction), tracks truncations, and persists a `RolloutItem::TurnContext` snapshot. It then streams model output via `drain_to_completed`, recording events and handling retries/backoff using the provider’s `stream_max_retries`.
- On errors:
  - Interruptions abort the compaction quietly.
  - Context-window excess trims the oldest history entries, notifying the user and retrying until the prompt fits or history is exhausted.
  - Other errors trigger exponential backoff with user-facing notifications; after max retries, an `Error` event is emitted.
- Upon success, the module reconstructs history by pulling the most recent assistant summary, collecting user messages (`collect_user_messages`), and calling `build_compacted_history` to produce a bridge message that links prior context to the new summary. It replaces session history, records a `RolloutItem::Compacted`, and announces completion via `AgentMessage`.
- Utility helpers:
  - `content_items_to_text` concatenates text segments while ignoring images.
  - `collect_user_messages` filters response items down to genuine user text (skipping session prefix markers).
  - `build_compacted_history` trims aggregated user text to 20k tokens (≈80k bytes), renders a templated bridge message, and appends it to the initial context.
  - `drain_to_completed` consumes the response stream, forwarding output events to history or rate-limit updates and returning the final token usage.

## Broader Context
- Compaction integrates tightly with `Session` history management; it assumes `Session::clone_history`, `record_into_history`, and `replace_history` maintain consistency between in-memory and persisted rollout state.
- The summarization prompt lives under `templates/compact`; changes to bridge wording or truncation thresholds should remain synchronized between this module and any user-facing documentation.
- Context can't yet be determined for multi-modal compaction; current logic skips image inputs. Future support for image-aware summaries would require extending both prompt content and `content_items_to_text`.

## Technical Debt
- None observed; the module addresses truncation, retries, and persistence coherently, with explicit TODOs tracked in neighboring modules (e.g., history management in `codex.rs`).

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../codex.rs.spec.md
