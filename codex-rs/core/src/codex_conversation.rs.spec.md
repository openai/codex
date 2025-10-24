## Overview
`core::codex_conversation` wraps the lower-level `Codex` façade in a simpler interface for callers that only need to submit operations and stream events. It is primarily used by `ConversationManager` and workspace front ends to manage conversations without exposing the full session internals.

## Detailed Behavior
- `CodexConversation` stores a `Codex` instance and exposes three async methods:
  - `submit(Op)` to enqueue a protocol operation and receive the generated submission ID.
  - `submit_with_id(Submission)` to push a pre-built submission (used rarely; intended to be phased out).
  - `next_event()` to await the next emitted `Event` from the agent.
- The struct is constructed via the crate-private `new` function in `ConversationManager::finalize_spawn`, ensuring only validated sessions become conversation handles.

## Broader Context
- Acts as the conversation handle returned by `ConversationManager::new_conversation`, allowing CLI/TUI code to interact with the agent while `ConversationManager` tracks lifecycle and fork/resume operations.
- Because it forwards directly to `Codex`, contract changes (e.g., backpressure semantics) must be maintained in both types. Specs for `ConversationManager` and front-end adapters should cross-reference this wrapper.
- Context can't yet be determined for future streaming abstractions; if a multi-tenant conversation pool emerges, the wrapper may evolve to incorporate more lifecycle metadata.

## Technical Debt
- The `submit_with_id` escape hatch remains due to legacy call sites. Removing it once all callers migrate to auto-generated IDs would simplify the API.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Eliminate `submit_with_id` once all clients rely on `submit` for ID generation.
related_specs:
  - ../mod.spec.md
  - ./codex.rs.spec.md
  - ./conversation_manager.rs.spec.md
