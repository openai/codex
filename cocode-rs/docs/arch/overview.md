# Architecture Overview

## System Layers

```
┌─────────────────────────────────────────────────────────────┐
│                      Application Layer                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   CLI       │  │   Session   │  │  app-server (IDE)   │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│  ┌─────────────────────────────────────────────────────────┐│
│  │  (Future: TUI)  - EventBroker, FrameScheduler pattern  ││
│  └─────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────┤
│                      Features Layer                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Hooks     │  │   Skills    │  │      Plugins        │  │
│  │ - PreTool   │  │ - /commit   │  │ - Marketplace       │  │
│  │ - PostTool  │  │ - /review   │  │ - Commands          │  │
│  │ - Session   │  │ - Custom    │  │ - Extensions        │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                         MCP Layer                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  MCP Types  │  │ MCP Client  │  │    MCP Server       │  │
│  │ - Protocol  │  │ - Transports│  │ - JSON-RPC stdio    │  │
│  │ - Messages  │  │ - Discovery │  │ - Tool export       │  │
│  │ - Tools     │  │ - Execution │  │ - Session bridge    │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                        Core Layer                            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐   │
│  │  Message │ │  Tools   │ │ Context  │ │  Agent Loop  │   │
│  │  Types   │ │(+BuiltIn)│ │  State   │ │   Driver     │   │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────┘   │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────────────────┐│
│  │  Prompt  │ │  Agent   │ │        Plan Mode Tools       ││
│  │ Builder  │ │ (Subagt) │ │  EnterPlan | ExitPlan | ...  ││
│  └──────────┘ └──────────┘ └──────────────────────────────┘│
├─────────────────────────────────────────────────────────────┤
│                     Execution Layer                          │
│  ┌─────────────────────┐  ┌─────────────────────────────┐   │
│  │      Sandbox        │  │        Shell                │   │
│  │   - Permission      │  │   - Command exec            │   │
│  │   - File access     │  │   - PTY support             │   │
│  └─────────────────────┘  └─────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                     Provider Layer (existing)                │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                    hyper-sdk                         │    │
│  │  OpenAI | Anthropic | Gemini | Volcengine | Zai     │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Key Features Coverage

### Slash Commands (cocode-skill)
- Unified skill/slash command system (like Claude Code v2.1.3+)
- User-invocable via `/command` syntax
- Loading from: Managed → User → Project → Plugin sources
- Examples: `/commit`, `/review-pr`, `/help`

### Plan Mode (cocode-tools + cocode-context)
- `EnterPlanModeTool` - Transitions to plan mode
- `ExitPlanModeTool` - Exits plan mode with approval
- `PermissionMode::Plan` - Read-only exploration
- Plan files stored in `~/.claude/plans/` with unique slugs

### Context Compaction (cocode-loop)
- Auto-compaction when context exceeds threshold (default 0.8)
- `PreCompact` hook for extensibility
- Summarization of older messages
- `CompactionStarted`/`CompactionCompleted` events

## Data Flow

### Single Turn Execution

```
User Input
    │
    ▼
┌───────────────┐
│ Agent Loop    │◄─────────────────┐
│ - Build prompt│                  │
│ - Call LLM    │                  │
└───────┬───────┘                  │
        │                          │
        ▼                          │
┌───────────────┐                  │
│ Stream LLM    │ StreamEvent      │
│ Response      │──────────────────┼──► UI (text deltas)
└───────┬───────┘                  │
        │ tool_use blocks          │
        ▼                          │
┌───────────────┐                  │
│ Tool Executor │                  │
│ - Concurrent  │                  │
│ - Sequential  │                  │
└───────┬───────┘                  │
        │ tool_result              │
        ▼                          │
┌───────────────┐                  │
│ Context Update│                  │
│ - Add results │                  │
│ - Check stop  │──────────────────┘
└───────────────┘     (continue if more tool calls)
        │
        ▼ (stop_reason != tool_use)
    Final Response
```

### Subagent Execution

```
Main Agent Loop
    │
    ▼ (Task tool call)
┌──────────────────┐
│ Subagent Manager │
│ - Find agent def │
│ - Fork context   │
│ - Filter tools   │
│ - Select model   │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Child Agent Loop │
│ - Own context    │
│ - Filtered tools │
│ - Own model      │
└────────┬─────────┘
         │
         ▼
    Subagent Result
         │
         ▼
    Main Agent continues
```

## Key Abstractions

### 1. ConversationMessage (extends hyper-sdk::Message)

```rust
struct ConversationMessage {
    inner: Message,           // hyper-sdk Message
    turn_id: Option<String>,  // Turn tracking
    uuid: String,             // Unique ID
    timestamp: i64,           // Creation time
    metadata: MessageMetadata,// Extended metadata
}
```

### 2. Tool Trait

```rust
#[async_trait]
trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self) -> ToolDefinition;
    fn concurrency_safety(&self) -> ConcurrencySafety;
    async fn execute(&self, input: Value, ctx: ToolContext) -> ToolOutput;
}
```

### 3. AgentLoop

```rust
struct AgentLoop {
    model: Arc<dyn Model>,
    tools: ToolRegistry,
    context: ConversationContext,
    config: LoopConfig,
}

impl AgentLoop {
    async fn run(&mut self, msg: ConversationMessage) -> Result<LoopResult>;
}
```

### 4. SubagentManager

```rust
struct SubagentManager {
    definitions: Vec<AgentDefinition>,
    running: HashMap<String, RunningAgent>,
}

impl SubagentManager {
    async fn spawn(&self, input: SpawnInput) -> Result<String>;
    async fn resume(&self, agent_id: &str) -> Result<String>;
}
```

## Integration with Existing Infrastructure

### hyper-sdk Types (Reuse)

| Type | Usage |
|------|-------|
| `Message` | Base message type |
| `ContentBlock` | Content blocks (text, tool_use, etc.) |
| `Role` | Message roles |
| `ToolDefinition` | Tool definitions for LLM |
| `ToolCall` | Parsed tool calls from response |
| `StreamEvent` | Streaming events |
| `Model` trait | LLM model abstraction |
| `Provider` trait | Provider abstraction |

### New Types Needed

| Type | Purpose |
|------|---------|
| `ConversationMessage` | Extended message with tracking |
| `Tool` trait | Tool execution abstraction |
| `ToolRegistry` | Tool registration and lookup |
| `ToolExecutor` | Parallel/sequential execution |
| `AgentLoop` | Main agent loop driver |
| `AgentDefinition` | Subagent configuration |
| `AppState` | Application state container |
| `PermissionContext` | Permission tracking |

## Error Handling

Follow cocode-error patterns:

```rust
#[stack_trace_debug]
#[derive(Snafu)]
pub enum LoopError {
    #[snafu(display("Tool execution failed: {name}"))]
    ToolFailed {
        name: String,
        source: ToolError,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("LLM API error"))]
    LlmError {
        source: HyperError,
        #[snafu(implicit)]
        location: Location,
    },

    // ... more variants
}
```

## MCP Architecture

### MCP Client Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    McpConnectionManager                      │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ Manages N McpClient instances (one per MCP server)   │   │
│  └──────────────────────────────────────────────────────┘   │
│                            │                                 │
│       ┌────────────────────┼────────────────────┐           │
│       ▼                    ▼                    ▼           │
│  ┌──────────┐        ┌──────────┐        ┌──────────┐      │
│  │ McpClient│        │ McpClient│        │ McpClient│      │
│  │ (stdio)  │        │ (sse)    │        │ (http)   │      │
│  └────┬─────┘        └────┬─────┘        └────┬─────┘      │
│       │                   │                   │             │
└───────┼───────────────────┼───────────────────┼─────────────┘
        │                   │                   │
        ▼                   ▼                   ▼
   ┌─────────┐         ┌─────────┐         ┌─────────┐
   │External │         │External │         │External │
   │MCP Srvr │         │MCP Srvr │         │MCP Srvr │
   └─────────┘         └─────────┘         └─────────┘
```

### MCP Tool Integration

```
Agent Loop (tool_use: "mcp__weather__get_forecast")
    │
    ├─ Parse tool name → (server="weather", tool="get_forecast")
    │
    ├─ Dispatch to McpToolHandler
    │
    └─ McpConnectionManager::call_tool(server, tool, args)
        │
        ├─ Emit McpToolCallBegin event
        │
        ├─ client.tools_call(tool, args)
        │
        ├─ Emit McpToolCallEnd event
        │
        └─ Return tool result to agent loop
```

### MCP Server Architecture (Three-Task Pattern)

```
┌─────────────────────────────────────────────────────────────┐
│                      MCP Server                              │
│                                                              │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │  Task 1: stdin  │  │ Task 2: process │  │Task 3:stdout│ │
│  │  reader         │  │ message handler │  │writer       │ │
│  │                 │  │                 │  │             │ │
│  │ Read JSON-RPC   │─▶│ Route to handler│─▶│ Write resp  │ │
│  │ from stdin      │  │ (init, call,...)│  │ to stdout   │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
│                              │                              │
│                              ▼                              │
│                    ┌─────────────────┐                     │
│                    │  Agent Session  │                     │
│                    │  (cocode/reply) │                     │
│                    └─────────────────┘                     │
└─────────────────────────────────────────────────────────────┘
```

### MCP Configuration Scopes (Priority Order)

| Priority | Scope | Location | Purpose |
|----------|-------|----------|---------|
| 1 | Enterprise | System policy | Organization-wide control |
| 2 | Local | `.cocode.local.toml` | Local directory overrides |
| 3 | Project | `.mcp.toml` | Project-level configuration |
| 4 | User | `~/.config/cocode/mcp.toml` | User settings |
| 5 | Plugin | Plugin manifests | Plugin-provided servers |

## UI Extensibility Architecture

### Event-Driven Communication

All UI frontends (CLI, TUI, IDE) communicate with core via event channels:

```
┌─────────────────────────────────────────────────────────────┐
│                        Core Layer                            │
│                                                              │
│  AgentLoop ──► mpsc::Sender<LoopEvent> ──┐                 │
│                                           │                 │
└───────────────────────────────────────────┼─────────────────┘
                                            │
              ┌─────────────────────────────┼─────────────────┐
              │                             ▼                 │
              │            ┌────────────────────────┐        │
              │            │  Event Router/Fan-out  │        │
              │            └────────────────────────┘        │
              │                    │    │    │               │
              │         ┌──────────┘    │    └──────────┐    │
              │         ▼               ▼               ▼    │
              │    ┌─────────┐    ┌─────────┐    ┌─────────┐│
              │    │  CLI    │    │  TUI    │    │app-srvr ││
              │    │Renderer │    │EventBus │    │JSON-RPC ││
              │    └─────────┘    └─────────┘    └─────────┘│
              │                        │              │      │
              │                        ▼              ▼      │
              │                   Terminal      IDE/Editor   │
              └──────────────────────────────────────────────┘
```

### app-server Protocol (IDE Integration)

Bidirectional JSON-RPC 2.0 over stdio for IDE/extension integration:

**Client → Server (Requests)**
```
Initialize     → Handshake with client info
TurnStart      → Send user input, stream responses
TurnInterrupt  → Cancel running turn
ConfigRead     → Read configuration
SkillsList     → List available skills
ModelList      → List available models
```

**Server → Client (Notifications)**
```
TurnStarted     → New turn began
ItemDelta       → Streaming content (text, reasoning, tool output)
ItemCompleted   → Item finished
ApprovalReq     → Request user permission
```

### TUI Patterns (for future implementation)

#### EventBroker (Pause/Resume Input)

```rust
// Enable external editor spawning without losing input
broker.pause_events();   // Release stdin for subprocess
// ... spawn external editor ...
broker.resume_events();  // Reconnect stdin
```

#### FrameScheduler (Efficient Redraws)

```rust
// Coalesce frame requests, honor 60 FPS limit
frame_requester.schedule_frame();  // Cloneable handle
// FrameScheduler task coalesces, sends single notification
```

#### Widget → Core Communication

```
Widget Layer
    │
    ├─ FrameRequester (clone) ──► FrameScheduler
    │
    └─ AppEventSender (clone) ──► mpsc::channel ──► App::handle_event()
```

### Key Extensibility Points

| Layer | Extension Point | Mechanism |
|-------|-----------------|-----------|
| Core | New event type | Add `LoopEvent` variant |
| MCP | New transport | Implement `McpTransport` trait |
| Features | New hook event | Add to `HookEventType` enum |
| App | New protocol | Implement message processor |
| TUI | New widget | Use `AppEventSender` + `FrameRequester` |

### Design Constraints

1. **No UI in core crates** - Core layer has zero UI dependencies
2. **Channel-based IPC** - All async communication via mpsc/broadcast
3. **Event immutability** - Events are read-only, create new events for mutations
4. **Graceful degradation** - Core works without any UI attached
