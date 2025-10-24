## Overview
`protocol::items` models the high-level items that make up a conversation turn. It unifies user messages, agent responses, reasoning traces, and web search metadata so downstream consumers can render rich transcripts or replay turns.

## Detailed Behavior
- `TurnItem` wraps four item types: `UserMessage`, `AgentMessage`, `Reasoning`, and `WebSearch`. Each variant carries an ID (generated on creation) and content tailored to its role.
- `UserMessageItem` stores the structured `UserInput` components that originated the turn. Helper methods expose concatenated text, associated image URLs, and conversions into the legacy `EventMsg` format for backward compatibility.
- `AgentMessageItem` collects `AgentMessageContent` entries (currently `Text`), provides a constructor mirroring `UserMessageItem::new`, and converts messages into legacy event deltas.
- `ReasoningItem` holds summary lines and optional raw reasoning dumps. The `as_legacy_events` helper emits reasoning events and, when requested, raw chain-of-thought events so UIs can toggle deeper insights.
- `WebSearchItem` stores the search query and converts to `WebSearchEnd` events with the stored identifier.
- `TurnItem::id` and `TurnItem::as_legacy_events` expose common operations across variants, ensuring callers can treat mixed turn item lists uniformly.

## Broader Context
- Serves as the bridge between modern turn-based representations and legacy event streams still used by some clients. As new event types land in `protocol.rs`, corresponding turn item variants should be added here to keep transcript rendering coherent.
- UUID generation for IDs relies on `uuid::Uuid::new_v4`; specs covering persistence or replay systems should note that these IDs are opaque.
- Context can't yet be determined for streaming reasoning variants beyond text; future expansions (e.g., structured reasoning graphs) will require extending the data model.

## Technical Debt
- None observed; helper methods keep legacy support contained without duplicating logic elsewhere.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./protocol.rs.spec.md
  - ./user_input.rs.spec.md
