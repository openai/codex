# `run_codex_convo` CLI Plan

## Goal

Single-command Rust utility that takes a user message, runs it through Codex app-server (JSON-RPC over stdio), and prints the streamed assistant output until the turn finishes.

## Assumptions

- User already configured Codex (auth, model defaults) so a fresh conversation can run without extra prompts.
- Project will live in its own cargo crate.

## Functional Requirements

1. Accept a single positional argument `user_message`; optional flags:
   - `--cwd`, `--model`, `--approval-policy`, `--sandbox` (pass-through overrides).
2. Launch `codex app-server` as a child process with stdio pipes.
3. Perform JSON-RPC handshake (`initialize`, `initialized`).
4. Start a conversation (`newConversation`) with optional overrides.
5. Send the user message (`sendUserMessage`).
6. Subscribe to the conversation (`addConversationListener`).
7. Stream `codex/event/*` notifications:
   - Print assistant text (`agent_message`, `agent_message_delta`).
   - Surface approvals/errors for visibility.
8. Stop once the server emits `task_complete` or `turn_aborted` for that request.
9. Shutdown child process gracefully (send EOF / kill on timeout).

## Implementation Outline

### 1. Workspace Setup

- Create new binary crate with `tokio` + `anyhow` + `serde` + `serde_json` + `clap` + `tracing`.
- Add helper module re-exporting Codex protocol types or lightweight copies for requests/responses (consider depending on `codex-rs` workspace crate if feasible).

### 2. Process & IO Management

- Use `tokio::process::Command` to spawn `codex-app-server` with piped stdin/stdout/stderr.
- Wrap stdio with framed readers/writers (`tokio_util::codec::LinesCodec` or manual line buffering) to handle newline-delimited JSON.
- Spawn task to log stderr from the child for troubleshooting.

### 3. JSON-RPC Layer

- Define structs for requests/responses using `serde`.
- Implement `RequestId` generator and helper to send requests and await matching responses (maintain pending map keyed by id, fulfilled when response arrives).
- Parse incoming messages: discriminate between responses (`result`/`error`) and notifications (`method` w/out `id`).

### 4. Conversation Flow

1. Send `initialize` with client metadata.
2. Post `initialized` notification.
3. Issue `newConversation` (with optional overrides) and capture `conversationId`.
4. Immediately call `addConversationListener`.
5. Send `sendUserMessage` payload containing provided text.
6. Track the request id of `sendUserMessage` to correlate events.

### 5. Streaming Output Rendering

- For `codex/event/agent_message` and `agent_message_delta`, print to stdout (buffer deltas to form cohesive message).
- Optionally colorize using `owo-colors` or similar.
- Surface `agent_reasoning` events as faint/optional output behind a `--show-reasoning` flag.
- If `exec_approval_request` or `apply_patch_approval_request` arrives, log notice that approvals were requested but auto-denied (since CLI cannot interact).

### 6. Completion Criteria

- Monitor events for `task_complete` or `turn_aborted` matching this conversation.
- Once received, flush output, collect final status, and exit with code 0 (or non-zero if `error` occurred).
- Ensure outstanding pending requests are dropped to avoid hanging tasks.

### 7. Graceful Shutdown

- After completion, send EOF to server stdin or issue `interruptConversation`/`archiveConversation` if desired.
- Await child termination with timeout; on failure, send kill signal.

### 8. Testing Strategy

- Unit-test JSON parsing and request builders.
- Write integration test spawning a mocked Codex server (or use real binary behind feature flag) verifying handshake + message flow.
- Add CLI smoke test using `assert_cmd` if Codex binary available.

## Enhancements (Later)

- Support multi-turn conversations (`run_codex_convo --repl`).
- Add `--plan` flag to render plan tool output as checklist.
- Stream structured logs to file for debugging.
- Accept stdin message body when positional arg omitted.

