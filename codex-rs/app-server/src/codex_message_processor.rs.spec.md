## Overview
`CodexMessageProcessor` is the JSON-RPC workhorse behind the VS Code app server. It owns the core auth/conversation managers, fan-outs incoming `ClientRequest`s, and translates Codex conversation events into responses, notifications, and approval prompts for the client.

## Detailed Behavior
- **State & construction**
  - Holds references to the shared `AuthManager`, `ConversationManager`, config, outgoing transport, and optional `codex-linux-sandbox` path.
  - Tracks subscription handles (`conversation_listeners`), active login server state (`active_login`), pending interrupt responses, and in-flight fuzzy-search cancellation tokens.
- **Request dispatch (`process_request`)**
  - Conversation lifecycle: `NewConversation`, `ResumeConversation`, `ArchiveConversation`, `ListConversations`, `ListModels`, and `Add/RemoveConversationListener`. New conversations derive runtime config via `derive_config_from_params`, while listing leverages `RolloutRecorder` and `extract_conversation_summary`.
  - Messaging: `SendUserMessage` and `SendUserTurn` map wire items into core `UserInput`, submit them through `Op::UserInput` / `Op::UserTurn`, and acknowledge immediately. `InterruptConversation` queues the request until a `TurnAborted` event arrives.
  - Tooling and helpers: `ExecOneOffCommand` runs commands in a sandbox consistent with policy defaults; `FuzzyFileSearch` deduplicates concurrent tokens and feeds results from `run_fuzzy_file_search`; `GitDiffToRemote` streams git status snapshots.
  - Authentication & config: manages API-key and ChatGPT login flows (spawning/canceling `run_login_server`, emitting `AuthStatusChangeNotification`s), surfaces auth status, rate limits, saved config (via `load_config_as_toml`), and user agent / info. `SetDefaultModel` persists overrides through `persist_overrides_and_clear_if_none`.
  - Unimplemented account APIs respond with JSON-RPC errors and an `INVALID_REQUEST` code.
- **Event handling**
  - Conversation listeners spawn background tasks that forward events to the client. `apply_bespoke_event_handling` intercepts `ApplyPatchApprovalRequest` and `ExecApprovalRequest`, forwards them to the UI, and submits the resulting approval (defaulting to denial on decode or transport errors).
  - Token usage events propagate `AccountRateLimitsUpdated` notifications; `TurnAborted` drains any queued interrupt replies.
  - Helper futures (`on_patch_approval_response`, `on_exec_approval_response`) deserialize replies, log failures, and convert them into corresponding `Op` submissions.
- **Utilities**
  - `derive_config_from_params` merges RPC-supplied overrides with CLI/environment defaults, converting JSON overrides to TOML via `json_to_toml`.
  - `extract_conversation_summary` distills rollout history into summaries displayed by the client (preferring plain user messages and trimming `USER_MESSAGE_BEGIN` prefixes).
  - `fetch_account_rate_limits` constructs a `BackendClient` from auth tokens and wraps errors in JSON-RPC payloads.

## Broader Context
- Works in tandem with the outer `MessageProcessor` (`./message_processor.rs.spec.md`), which handles handshake/initialization before delegating every request here.
- Collaborates with `OutgoingMessageSender` to multiplex responses, notifications, and new server-initiated requests (`./outgoing_message.rs.spec.md`).
- Shares fuzzy-search and model metadata helpers with local modules (`./fuzzy_file_search.rs.spec.md`, `./models.rs.spec.md`) and core services (exec, configuration, rollout, approvals).

## Technical Debt
- Approval forwarding tasks (`ApplyPatchApprovalRequest`, `ExecApprovalRequest`) lack explicit timeouts, so hung clients can leave background tasks alive indefinitely.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add timeouts when awaiting approval responses to prevent orphaned tasks.
related_specs:
  - ./message_processor.rs.spec.md
  - ./outgoing_message.rs.spec.md
  - ./fuzzy_file_search.rs.spec.md
  - ./models.rs.spec.md
  - ./error_code.rs.spec.md
