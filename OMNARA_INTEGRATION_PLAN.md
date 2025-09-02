# Omnara Integration for Codex CLI

## Overview
This document describes the Omnara integration for the OpenAI Codex CLI (Rust implementation). The integration allows the Codex agent to communicate with the Omnara dashboard in real-time, enabling remote users to see agent messages and provide input.

## Implementation Status: ✅ COMPLETE

### What's Implemented
- ✅ OmnaraClient module with full API integration
- ✅ Configuration via CLI args, env vars, and config file  
- ✅ Agent message sending with message ID tracking
- ✅ User message sending from local TUI
- ✅ Request user input on task completion OR when task not running
- ✅ Polling for remote user responses (24 hours, 5-second intervals)
- ✅ Remote messages displayed in TUI and processed by agent
- ✅ Deterministic message handling (no race conditions, no delays)
- ✅ Comprehensive logging to `~/.omnara/codex_wrapper/`
- ✅ Initial session message with automatic polling on startup
- ✅ Polling cancellation when local user input arrives
- ✅ Session end API call when user exits

## System Architecture

### Event Flow in Codex
1. **Core Agent Processing** (`core/src/codex.rs`):
   - Processes user input and generates AI responses
   - Sends events through channels to the UI layer
   - Events are sent in a deterministic order:
     ```
     TaskStarted → AgentMessage(s) → [Tool Events] → TaskComplete(last_agent_message)
     ```

2. **UI Event Handling** (`tui/src/chatwidget.rs`):
   - Receives events synchronously via `handle_event()`
   - Each event type has a dedicated handler (e.g., `on_agent_message`, `on_task_complete`)
   - Spawns async tasks for network operations (Omnara API calls)

3. **The Synchronization Challenge & Solution**:
   - Problem: `on_agent_message` spawns async task to send to Omnara
   - `on_task_complete` might execute before the async send completes
   - Solution: Store `JoinHandle` and await it for deterministic behavior (NO DELAYS!)

## Omnara Integration Design

### Key Components

1. **OmnaraClient** (`core/src/omnara_client.rs`):
   - Manages HTTP communication with Omnara API
   - Stores state: session ID, last message ID (Arc<Mutex>), polling status
   - Provides async methods for all operations
   - Logs all interactions to `~/.omnara/codex_wrapper/[session_id].log`

2. **Message Flow**:
   ```
   Session Start → Send initial message → Poll for 24 hours
   Agent Message → Send to Omnara (get message_id) → Store ID
   Task Complete → Wait for send → Request user input → Poll for response
   User types locally → Cancel ALL polling (handle + flag) → Send to Omnara
   ```

3. **API Endpoints Used**:
   - `POST /api/v1/messages/agent` - Send agent messages
   - `POST /api/v1/messages/user` - Send user messages  
   - `PATCH /api/v1/messages/{id}/request-input` - Mark message as requiring input
   - `GET /api/v1/messages/pending` - Poll for user responses
   - `POST /api/v1/sessions/end` - End session when user exits

### Deterministic Message Handling

**The Solution**:
```rust
// In on_agent_message:
self.last_agent_send_handle = Some(tokio::spawn(async move {
    omnara_clone.send_message(message, false).await
}));

// In on_task_complete:
if let Some(handle) = self.last_agent_send_handle.take() {
    let _ = handle.await;  // Wait for send to complete
    // Now message ID is guaranteed to be stored
    omnara_clone.handle_task_complete().await
}
```

**Why This Works**:
1. `JoinHandle` represents the spawned task
2. Awaiting it blocks until the task completes
3. The message ID is stored inside `send_message` before it returns
4. No arbitrary delays or race conditions

## Configuration

### CLI Arguments (Highest Priority)
```bash
cargo run --bin codex -- --omnara-api-key "key" --omnara-api-url "url" --omnara-session-id "uuid"
```

### Environment Variables
```bash
export OMNARA_API_KEY="your-key"
export OMNARA_API_URL="https://agent-dashboard-mcp.onrender.com"  # optional
export OMNARA_SESSION_ID="uuid"  # optional, auto-generates
```

### Config File (`~/.codex/config.toml`)
```toml
[omnara]
api_key = "your-key"
api_url = "https://agent-dashboard-mcp.onrender.com"  
session_id = "uuid"
```

Priority: CLI args > Environment vars > Config file

## Message Protocol

### Agent Messages
```json
{
  "agent_instance_id": "session-uuid",
  "content": "message text",
  "requires_user_input": false,
  "agent_type": "codex"
}
```
Response includes `message_id` which we store for later use.

### User Messages  
```json
{
  "agent_instance_id": "session-uuid",
  "content": "user input",
  "mark_as_read": true
}
```

### Request User Input
```
PATCH /api/v1/messages/{message_id}/request-input
```
Called on the last agent message when task completes.

### Polling Response
```json
{
  "agent_instance_id": "...",
  "messages": [
    {
      "id": "...",
      "content": "user message",
      "sender_type": "USER",  // Note: uppercase, but we don't filter by this
      "created_at": "...",
      "requires_user_input": false
    }
  ],
  "status": "ok"
}
```

**Important**: Following Python SDK, we return ANY messages without filtering by sender_type.

## Debugging

### Enable Debug Logs
```bash
RUST_LOG=info cargo run --bin codex -- --omnara-api-key "key"
```

### Check Omnara Logs
```bash
tail -f ~/.omnara/codex_wrapper/*.log
```

### Debug Messages to Look For
- `DEBUG: on_agent_message called with:` - Shows when agent messages arrive
- `DEBUG: Sent agent message ... with ID:` - Shows successful sends with IDs
- `DEBUG: on_task_complete called` - Shows task completion
- `DEBUG: Requesting user input for message ID:` - Shows which ID is used

## Known Issues & Solutions

1. **Duplicate Key Constraint (500 error)**:
   - Cause: Race condition with multiple rapid requests
   - Solution: Deterministic sending reduces this
   - Server fix needed: Use "get or create" pattern

2. **User Messages Before Agent Exists**:
   - Cause: User types before first agent message
   - Solution: First agent message creates instance with `agent_type`

3. **End Session Not Completing**:
   - Cause: App exits before async end_session task completes
   - Solution: Spawn nested async tasks - one to call end_session, another to wait for it and then send ExitRequest
   - Also added sync_all() calls to ensure logs are flushed to disk

## Testing

1. **Basic Flow**:
   ```bash
   cargo run --bin codex -- --omnara-api-key "key"
   # Type a message
   # See it appear in Omnara dashboard
   # Agent responds
   # Omnara shows "waiting for input"
   ```

2. **Remote Input**:
   - Type in Omnara dashboard
   - Should appear in local CLI
   - Agent processes it normally

## Code Locations

- **Client Implementation**: `core/src/omnara_client.rs`
- **Integration Point**: `tui/src/chatwidget.rs` 
- **Configuration**: `core/src/config.rs`, `core/src/config_types.rs`
- **CLI Arguments**: `tui/src/cli.rs`
- **Main Entry**: `tui/src/lib.rs` (creates client from config)

## Key Insights for Future Development

1. **Event Order Matters**: TaskComplete always comes after all AgentMessages
2. **Async Requires Care**: Always track JoinHandles for determinism (NO DELAYS!)
3. **State is Distributed**: Some in OmnaraClient, some in ChatWidget
4. **Logging is Critical**: Debug logs essential for understanding timing
5. **API Compatibility**: Must match Python SDK's request/response format exactly
6. **No Arbitrary Delays**: We wait for actual operations, not fixed timeouts
7. **TaskComplete Contains Info**: The event includes `last_agent_message` field
8. **Agent Instance Creation**: First agent message creates the instance with `agent_type`
9. **Why Async**: UI must remain responsive; blocking network calls would freeze terminal
10. **Session ID**: Must be valid UUID, auto-generates if not provided

## Critical Implementation Lessons

### Always Follow the Python SDK
The Python SDK is the reference implementation. Don't make assumptions about API behavior:
- **Polling**: Python SDK doesn't filter by sender_type, neither should we
- **Message IDs**: Must pass `last_read_message_id` when polling (not just agent_instance_id)
- **Response Handling**: Return all messages, don't filter

### Polling Implementation Details
1. **Request user input** on the last message: `PATCH /api/v1/messages/{id}/request-input`
2. **Poll with last_read_message_id**: Critical for getting new messages
3. **Don't filter messages**: API returns `sender_type: "USER"` (uppercase) but Python SDK doesn't filter
4. **Display in TUI**: Use `AppEvent::InsertHistoryCell` to show remote messages in terminal
5. **Polling duration**: 24 hours with 5-second intervals (matching Python SDK defaults)

### Common Pitfalls
- Assuming lowercase "user" when API returns uppercase "USER"
- Forgetting to pass `last_read_message_id` when polling
- Not displaying remote messages in the TUI (just sending to agent isn't enough)
- Making assumptions instead of checking Python SDK implementation

## Future Improvements

1. **Remove Debug Logs**: Clean up tracing::info! calls after stabilization
2. **Error Recovery**: Add retry logic with exponential backoff
3. **WebSocket Support**: Move from polling to real-time bidirectional communication
4. **Message History**: Load previous conversation on reconnect
5. **Multiple Sessions**: Support concurrent sessions with different IDs