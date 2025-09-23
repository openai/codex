# Codex Chrome Extension - Research Document (Based on Actual Code)

## Actual Codex-rs Architecture Analysis

### Core Components Found in Codebase

#### 1. Main Orchestration (`codex-rs/core/src/codex.rs`)

The actual system uses a **queue-based architecture** with:
- **Submission Queue (SQ)**: Incoming requests via `Submission` enum
- **Event Queue (EQ)**: Outgoing events via `Event` enum

**Key Structs to Preserve:**
```rust
// Main interface
struct Codex {
    next_id: AtomicU64,
    tx_sub: Sender<Submission>,
    rx_event: Receiver<Event>,
}

// Session management
struct Session {
    conversation_id: ConversationId,
    mcp_connection_manager: McpConnectionManager,
    session_manager: ExecSessionManager,
    unified_exec_manager: UnifiedExecSessionManager,
    rollout: Mutex<Option<RolloutRecorder>>,
    state: Mutex<State>,
}
```

#### 2. Protocol System (`codex-rs/protocol/src/protocol.rs`)

**Core Message Types:**
```rust
// User submissions
enum Op {
    UserInput { item: InputItem },
    UserTurn { context: TurnContext },
    OverrideTurnContext { context: TurnContext },
    Interrupt(Interrupt),
    // ... other operations
}

// System events
enum EventMsg {
    TaskStarted(TaskStartedEvent),
    TaskComplete(TaskCompleteEvent),
    AgentMessage(AgentMessageEvent),
    AgentMessageDelta(AgentMessageDeltaEvent),
    ExecCommandBegin(ExecCommandBeginEvent),
    ExecCommandEnd(ExecCommandEndEvent),
    // ... many more event types
}
```

#### 3. Model Client (`codex-rs/core/src/client.rs`)

**Key Components:**
```rust
struct ModelClient {
    config: Arc<Config>,
    auth_manager: Option<Arc<AuthManager>>,
    client: reqwest::Client,
    provider: ModelProviderInfo,
    conversation_id: ConversationId,
    effort: Option<ReasoningEffortConfig>,
}
```

#### 4. Tool System (`codex-rs/core/src/openai_tools.rs`)

**Actually Implemented Tools:**
- `exec_command` - Execute shell commands with streaming output
- `write_stdin` - Write to stdin of running commands
- `local_shell` - Built-in shell execution
- `unified_exec` - Experimental unified execution
- `view_image` - View image files
- `update_plan` - Update task planning
- `web_search` - Web search capability
- Apply patch tools (structured and freeform)
- MCP tool integration

### Key Differences from Initial Analysis

1. **No "AgentSystem" class** - The system uses `Codex` and `Session` structs
2. **No "QueryProcessor" class** - Uses `parse_command()` function
3. **No "ReasoningEngine" class** - Reasoning is handled by the model provider
4. **No "StateManager" class** - State is managed within `Session`
5. **Queue-based architecture** instead of direct method calls

### Components to Convert for Chrome Extension

#### Core System (Must Preserve Logic)

1. **Queue Architecture**:
   ```typescript
   // TypeScript conversion
   class CodexChromeAgent {
     private submissionQueue: Submission[] = [];
     private eventQueue: Event[] = [];

     async submitOperation(op: Op): Promise<void> {
       // Queue submission
     }

     async getNextEvent(): Promise<Event | null> {
       // Dequeue event
     }
   }
   ```

2. **Session Management**:
   ```typescript
   interface Session {
     conversationId: string;
     mcpConnectionManager?: McpConnectionManager; // Optional for Chrome
     state: SessionState;
     turnContext: TurnContext;
   }
   ```

3. **Protocol Messages**:
   ```typescript
   // Direct port from Rust
   type Op =
     | { type: 'UserInput', item: InputItem }
     | { type: 'UserTurn', context: TurnContext }
     | { type: 'Interrupt', reason: string };

   type EventMsg =
     | { type: 'TaskStarted', data: TaskStartedEvent }
     | { type: 'AgentMessage', data: AgentMessageEvent }
     | { type: 'ExecCommandBegin', data: ExecCommandBeginEvent }
     // ... etc
   ```

### Tool Replacements for Chrome Extension

| Codex-rs Tool | Chrome Extension Equivalent | Purpose |
|--------------|----------------------------|---------|
| `exec_command` | `executeInTab` | Run JavaScript in tab context |
| `local_shell` | `chromeDevTools` | Execute DevTools commands |
| `view_image` | `captureTab` | Screenshot tab content |
| `file_search` | `searchTabs` | Search across open tabs |
| `apply_patch` | `modifyDOM` | Modify page DOM |
| `web_search` | `webSearch` | Search web (keep same) |

### Architecture Adaptation Strategy

#### 1. Message Flow Conversion
```
Rust Flow:
User Input → Submission Queue → Session Processing → Model API → Event Queue → UI

Chrome Extension Flow:
Side Panel → Background Worker → Content Script → Web Page → Response → Side Panel
```

#### 2. Concurrency Model
- **Rust**: Uses tokio async runtime with channels
- **Chrome**: Use Service Worker with Chrome message passing

#### 3. Storage Adaptation
- **Rust**: File system and in-memory state
- **Chrome**: Chrome Storage API (local, session, sync)

### Key Files for Reference

1. **Core Logic**:
   - `codex-rs/core/src/codex.rs` - Main orchestration (4,600+ lines)
   - `codex-rs/core/src/client.rs` - Model client (1,400+ lines)
   - `codex-rs/core/src/exec.rs` - Execution logic (400+ lines)

2. **Protocol**:
   - `codex-rs/protocol/src/protocol.rs` - All message types (1,400+ lines)
   - `codex-rs/protocol/src/models.rs` - Model definitions (400+ lines)

3. **Tools**:
   - `codex-rs/core/src/openai_tools.rs` - Tool definitions (1,200+ lines)
   - `codex-rs/core/src/exec_command/` - Command execution details

### Implementation Priority

#### Phase 1: Core Message System
1. Port `Submission` and `Event` types
2. Implement queue-based message passing
3. Create Chrome message adapter

#### Phase 2: Session Management
1. Port `Session` struct
2. Adapt for Chrome storage
3. Implement turn context management

#### Phase 3: Tool Framework
1. Create browser tool interface
2. Implement tab management tools
3. Add content script injection

#### Phase 4: Model Integration
1. Port model client for browser
2. Handle streaming responses
3. Implement token counting

### Technical Challenges

1. **No Direct File System**: All file operations must become browser operations
2. **Async Model**: Chrome's event-driven model vs Rust's async/await
3. **Memory Constraints**: Chrome extensions have memory limits
4. **Security**: Content Security Policy restrictions

### Preserved Naming Conventions

Critical names to maintain:
- `Submission`, `Op`, `Event`, `EventMsg`
- `TurnContext`, `InputItem`, `ConversationId`
- `ExecCommandBeginEvent`, `ExecCommandEndEvent`
- `AgentMessage`, `AgentMessageDelta`
- Tool names: `exec_command`, `update_plan`, etc.

### Dependencies Not Needed

Components to skip in Chrome extension:
- `linux-sandbox/` - Not applicable in browser
- `mcp-*` - Optional, can add later if needed
- `login/` - Use Chrome's identity API
- `file-search/` - Replace with tab search
- `ollama/` - Local LLM not supported
- `tui/` - Replaced by Svelte UI