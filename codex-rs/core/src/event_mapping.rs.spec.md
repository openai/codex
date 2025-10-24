## Overview
`core::event_mapping` converts protocol `ResponseItem`s into higher-level `TurnItem`s for front-end consumption. It filters system-prefixed messages, unwraps reasoning content, and extracts user inputs or agent outputs in a form compatible with legacy event processing.

## Detailed Behavior
- `parse_turn_item` matches on `ResponseItem` variants:
  - User messages call `parse_user_message`, which skips session prefix markers (environment context, user instructions), converts text into `UserInput::Text`, and image blocks into `UserInput::Image`. Output text inside a user message is warned and ignored.
  - Assistant messages convert each text block into `AgentMessageContent::Text`.
  - Reasoning entries aggregate summary text and optional raw content (`ReasoningItemContent`) into `ReasoningItem`.
  - Web search calls produce `WebSearchItem` when the action is a search query.
  - Other response types return `None`, signaling no turn item representation.
- Helpers:
  - `is_session_prefix` detects session bootstrap messages to prevent duplicating them as user content.
  - `parse_agent_message` warns on unexpected content (e.g., non-text segments) but still captures text.
- Tests cover parsing user messages with images, assistant text, reasoning summaries, and web search events, ensuring the conversion logic aligns with downstream expectations.

## Broader Context
- `ConversationHistory` depends on `parse_turn_item` when distinguishing user vs. assistant messages, especially for compaction and history truncation.
- UI layers rely on `TurnItem` variants to render transcripts; maintaining this mapping is critical when new `ResponseItem` variants are introduced.
- Context can't yet be determined for richer agent outputs (e.g., tables, multi-modal content). When those arrive, new branches may be required to avoid warning spam and to provide structured turn items.

## Technical Debt
- Warning logs for unexpected content items could become noisy if upstream APIs add new variants. Adding metrics or structured logging would help monitor drift without flooding logs.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Replace plain `warn!` calls with structured telemetry once extended content types are introduced.
related_specs:
  - ./conversation_history.rs.spec.md
  - ../protocol/src/items.rs.spec.md
