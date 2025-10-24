## Overview
`protocol::conversation_id` wraps UUID v7 identifiers in a strongly typed `ConversationId`. It centralizes creation, parsing, and serialization so conversations have stable, opaque identifiers across processes.

## Detailed Behavior
- Stores an internal `Uuid` and exposes `ConversationId::new()` (using `Uuid::now_v7`) and `ConversationId::from_string` for parsing externally supplied IDs.
- Implements `Default`, `Display`, `Serialize`, and `Deserialize` to integrate seamlessly with JSON payloads and logging. Serialization emits the canonical string form; deserialization validates input before constructing the type.
- Provides a custom `JsonSchema` implementation that delegates to the string schema so generated schemas remain compatible with consumers expecting string UUIDs.
- Includes a unit test ensuring the default value is not the nil UUID, guarding against accidental regressions in the generator.

## Broader Context
- Used throughout the protocol to correlate submissions, events, and history entries. Keeping the type opaque discourages ad-hoc string handling in downstream code.
- UUID v7 provides monotonic ordering, which can aid in storage or log correlation; future specs describing persistence layers should note this assumption.
- Context can't yet be determined for cross-service identifiers (e.g., sharded deployments); extensions can add new constructors without breaking the existing API.

## Technical Debt
- None observed; the wrapper fully encapsulates UUID lifecycle and provides required trait implementations.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
