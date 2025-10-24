## Overview
`core::conversation_manager` oversees the lifecycle of Codex conversations. It spawns new sessions, resumes or forks existing histories, tracks active conversations in memory, and validates the initial handshake (`SessionConfigured`) before handing off control to callers.

## Detailed Behavior
- Stores conversations in an `Arc<RwLock<HashMap<ConversationId, Arc<CodexConversation>>>>`, allowing concurrent reads while serializing mutations.
- `ConversationManager::new` accepts an `AuthManager` and `SessionSource`, which are reused for every spawned session. `with_auth` is a test helper that seeds a fixed `CodexAuth`.
- `new_conversation` constructs a fresh `Config`, delegates to `spawn_conversation`, and returns `NewConversation`, packing the `CodexConversation`, `ConversationId`, and the first `SessionConfiguredEvent`.
- `spawn_conversation` invokes `Codex::spawn` with `InitialHistory::New`, while `resume_conversation_from_rollout` and `fork_conversation` call the same entry point with histories derived from rollout logs.
- `finalize_spawn` validates that the first event emitted by the new session is `SessionConfigured` with the empty submission ID (`INITIAL_SUBMIT_ID`) before registering the conversation. Failure to meet this contract raises `CodexErr::SessionConfiguredNotFirstEvent`, preventing partially initialized sessions from leaking.
- `get_conversation` fetches a conversation handle, returning `CodexErr::ConversationNotFound` when absent. `remove_conversation` drops a session from the manager’s map, returning the `Arc` so callers can decide whether to keep listening.
- `fork_conversation` loads rollout history, truncates it via `truncate_before_nth_user_message`, and spawns a new session that inherits the caller’s `Config`.
- `truncate_before_nth_user_message` walks rollout items, counting only user message inputs (ignoring session prefix items such as instructions or environment context). It returns `InitialHistory::New` when the requested message index is out of range; otherwise it slices the rollout up to—but excluding—the nth user message.

## Broader Context
- Higher-level services (CLI, app-server) rely on `ConversationManager` to enforce startup ordering, share `AuthManager` state across sessions, and provide safe forking/resume semantics. Specs for those services should reference the handshake validation performed here.
- Rollout management depends on `codex-core`’s rollout recorder; when the recorder schema changes, this manager’s resume/fork helpers must remain compatible.
- Context can't yet be determined for eviction policies or persistence of inactive conversations; today the map grows as new conversations spawn until callers explicitly remove entries.

## Technical Debt
- Conversation storage is unbounded and lacks eviction; when many sessions accumulate it may become necessary to impose limits or persist handles to disk.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add lifecycle hooks or eviction policies so long-lived processes do not retain unbounded conversation handles.
related_specs:
  - ./codex.rs.spec.md
  - ./codex_conversation.rs.spec.md
  - ../rollout/mod.spec.md
