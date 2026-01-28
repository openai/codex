# Implementation Plan

## Phase Overview

| Phase | Focus | Deliverables |
|-------|-------|--------------|
| 1 | Foundation | message, tools, context crates |
| 2 | Core Loop | loop, prompt crates |
| 3 | Execution | sandbox, shell |
| 4 | Multi-Agent | agent crate, Task tool |
| 4.5 | Advanced Execution | executor crate (iterative, collab) |
| 5 | MCP | mcp-types, mcp-client, mcp-server |
| 6 | Application | session, cli, integration |

## Phase 1: Foundation

### Goals
- Establish core type definitions
- Create crate structure
- Integrate with hyper-sdk

### Tasks

#### 1.1 Create cocode-message crate
```
core/message/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── conversation.rs  # ConversationMessage
    ├── metadata.rs      # MessageMetadata
    └── builders.rs      # Message builders
```

**Key types:**
- `ConversationMessage` - Extended message with tracking
- `MessageMetadata` - Turn ID, UUID, timestamp
- Builder functions for common message types

**Integration:**
- Wraps `hyper_sdk::Message`
- Preserves all hyper-sdk content block types
- Adds cocode-specific metadata

#### 1.2 Create cocode-tools crate
```
core/tools/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── trait.rs        # Tool trait (5-stage pipeline)
    ├── registry.rs     # ToolRegistry
    ├── context.rs      # ToolContext
    ├── output.rs       # ToolOutput, ContextModifier
    ├── permission.rs   # PermissionContext, PermissionResult
    ├── file/           # Read, Write, Edit, Glob, Grep
    ├── shell/          # Bash
    ├── web/            # WebFetch, WebSearch
    ├── user/           # AskUserQuestion
    ├── plan/           # EnterPlanMode, ExitPlanMode
    └── task/           # Task, TaskOutput, KillShell
```

**Key types:**
- `Tool` trait - 5-stage execution pipeline (enabled → permissions → validation → execution → result mapping)
- `ToolRegistry` - registration and lookup
- `ToolContext` - execution context with state
- `ConcurrencySafety` - parallel execution marker
- Built-in tool implementations

**Tool configuration for subagents:**
- `tools: Option<Vec<String>>` - allow-list (use `["*"]` for all)
- `disallowed_tools: Option<Vec<String>>` - deny-list

#### 1.3 Create cocode-context crate
```
core/context/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── conversation.rs # ConversationContext
    ├── state.rs        # AppState
    ├── permission.rs   # PermissionContext
    └── file_state.rs   # ReadFileState
```

**Key types:**
- `ConversationContext` - message history, tokens
- `AppState` - global application state
- `PermissionContext` - permission tracking
- `PermissionMode` - default/plan/bypass

#### 1.4 Create cocode-skill crate
```
core/skill/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── command.rs      # SkillPromptCommand
    ├── source.rs       # SkillSource, LoadedFrom enums
    ├── loader.rs       # Skill loading with fail-open semantics
    ├── scanner.rs      # SkillScanner with symlink safety
    ├── validator.rs    # Field validation
    ├── outcome.rs      # SkillLoadOutcome (partial success)
    ├── interface.rs    # SkillInterface (SKILL.toml metadata)
    ├── bundled.rs      # Bundled skills with fingerprinting
    └── bundled/        # Embedded bundled skills
        └── ...
```

**Key types:**
- `SkillPromptCommand` - unified skill representation
- `SkillSource` - configuration source (Builtin, Bundled, User, Project, etc.)
- `SkillLoadOutcome` - partial success with errors collection
- `SkillScanner` - safe traversal with cycle detection
- `SkillInterface` - optional UI metadata from SKILL.toml

**Safety features (from codex-rs patterns):**
- Field length limits (MAX_NAME_LEN, MAX_DESCRIPTION_LEN, etc.)
- Traversal limits (MAX_SCAN_DEPTH, MAX_SKILLS_DIRS_PER_ROOT)
- Symlink cycle detection via canonical path tracking
- Fail-open error handling (one bad skill doesn't break others)

**Bundled skills:**
- Embedded at build time via `include_dir!`
- Fingerprint-based update detection
- Automatic installation to `~/.cocode/skills-bundled/`

**Dependencies:**
- `include_dir` - embed skill files at build time
- `sha2` - fingerprint computation
- `toml` - SKILL.toml parsing

### Verification
```bash
cargo check -p cocode-message --manifest-path cocode-rs/Cargo.toml
cargo check -p cocode-tools --manifest-path cocode-rs/Cargo.toml
cargo check -p cocode-context --manifest-path cocode-rs/Cargo.toml
cargo check -p cocode-skill --manifest-path cocode-rs/Cargo.toml
cargo test -p cocode-message --manifest-path cocode-rs/Cargo.toml
cargo test -p cocode-skill --manifest-path cocode-rs/Cargo.toml
```

## Phase 2: Core Loop

### Goals
- Implement agent loop driver with StreamingToolExecutor
- Add prompt building
- Streaming tool execution (execute tools DURING API streaming, not after)

### Tasks

#### 2.1 Create cocode-prompt crate
```
core/prompt/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── builder.rs      # PromptBuilder
    ├── system.rs       # System prompt templates
    ├── reminder.rs     # System reminder formatting
    └── templates/      # Prompt template strings
```

**Key features:**
- System prompt generation with tool descriptions
- System reminder formatting (`<system-reminder>` tags)
- User/system context injection

#### 2.2 Create cocode-loop crate
```
core/loop/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── config.rs       # LoopConfig
    ├── event.rs        # LoopEvent (complete event types)
    ├── driver.rs       # AgentLoop
    ├── executor.rs     # StreamingToolExecutor (parallel/sequential)
    ├── result.rs       # LoopResult
    └── compaction.rs   # Micro-compaction + auto-compaction
```

**Key features:**
- StreamingToolExecutor - execute tools DURING API streaming
- Concurrency-safe execution (parallel for read-only, sequential for writes)
- Recursive turn-based execution
- Streaming event emission
- Micro-compaction (remove low-value tool results)
- Auto-compaction (summarize when context approaching limit)
- Model fallback with orphaned message handling

### Verification
```bash
cargo check -p cocode-loop --manifest-path cocode-rs/Cargo.toml
# Integration test with mock model
cargo test -p cocode-loop --manifest-path cocode-rs/Cargo.toml
```

## Phase 3: Execution

### Goals
- Add sandboxing
- Command execution

### Tasks

#### 3.1 Create cocode-sandbox crate
```
exec/sandbox/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── config.rs       # SandboxConfig
    ├── checker.rs      # PermissionChecker
    └── platform/       # Platform-specific
        ├── mod.rs
        ├── unix.rs
        └── windows.rs
```

#### 3.2 Create cocode-shell crate
```
exec/shell/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── executor.rs     # ShellExecutor
    ├── command.rs      # Command parsing
    ├── background.rs   # BackgroundProcess
    └── readonly.rs     # Read-only command detection
```

### Verification
```bash
cargo check -p cocode-sandbox --manifest-path cocode-rs/Cargo.toml
cargo check -p cocode-shell --manifest-path cocode-rs/Cargo.toml
cargo test -p cocode-sandbox --manifest-path cocode-rs/Cargo.toml
```

## Phase 4: Multi-Agent

### Goals
- Subagent spawning and management
- 4 built-in agent definitions (Bash, general-purpose, Explore, Plan)
- Task tool implementation with tools[]/disallowed_tools[] configuration
- Background mode support (agent vs bash mechanisms)

### Tasks

#### 4.1 Create cocode-subagent crate
```
core/subagent/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── definition.rs   # AgentDefinition (tools[], disallowed_tools[])
    ├── manager.rs      # SubagentManager
    ├── spawn.rs        # SpawnInput, spawning logic
    ├── filter.rs       # filter_tools_for_agent() (3-layer filtering)
    ├── context.rs      # ChildToolUseContext
    ├── definitions/    # 4 built-in agent definitions
    │   ├── mod.rs      # builtin_agents()
    │   ├── bash.rs     # Bash agent
    │   ├── explore.rs  # Explore agent (inherits model, read-only)
    │   ├── plan.rs     # Plan agent (architecture)
    │   └── general.rs  # general-purpose agent
    ├── prompts/        # Agent system prompts
    │   └── mod.rs
    ├── background.rs   # Background aggregation (local_agent)
    ├── signal.rs       # Background signal map (Ctrl+B)
    └── transcript.rs   # Sidechain transcript (JSONL)
```

**Built-in agents (4):**
1. **Bash** - Command execution specialist (tools: ["Bash"])
2. **general-purpose** - Full capability with context (tools: ["*"], disallowed: ["Task"])
3. **Explore** - Fast codebase exploration (inherits model, read-only, bypass permissions)
4. **Plan** - Software architect (read-only, Glob/Grep/Read/Bash)

Additional agents (statusline-setup, claude-code-guide) come from settings/plugins.

#### 4.2 Add Task Tool
Add to cocode-tools:
```rust
// core/tools/src/agent/task.rs
pub struct TaskTool { ... }
```

### Verification
```bash
cargo check -p cocode-subagent --manifest-path cocode-rs/Cargo.toml
# Test subagent spawning
cargo test -p cocode-subagent --manifest-path cocode-rs/Cargo.toml
```

## Phase 4.5: Advanced Execution Modes

### Goals
- Independent agent execution (no parent context)
- Iterative multi-run execution
- Multi-agent coordination with explicit communication

See [execution-modes.md](execution-modes.md) for detailed architecture.

### Tasks

#### 4.5.1 Create cocode-executor crate
```
core/executor/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── agent_executor.rs    # AgentExecutor (base primitive)
    ├── iterative.rs         # IterativeExecutor (multi-run)
    ├── coordinator.rs       # AgentCoordinator (multi-agent)
    ├── condition.rs         # IterationCondition (Count/Duration)
    ├── context.rs           # IterationContext, IterationRecord
    ├── summarizer.rs        # Iteration summarization (LLM/file-based)
    └── tools/               # Collab tools
        ├── mod.rs
        ├── spawn_agent.rs   # Spawn new agent
        ├── send_input.rs    # Multi-turn communication
        ├── wait.rs          # Wait for agent completion
        └── close_agent.rs   # Shutdown agent
```

**Key types:**

1. **AgentExecutor** - Base primitive for independent agent
   - Runs full agent without parent context
   - Foundation for iterative and collab patterns

2. **IterativeExecutor** - Multi-run execution
   - IterationCondition: Count(N) or Duration(T)
   - Context passing via prompt injection
   - Continue-on-error semantics
   - Git commit integration (optional)
   - LLM-based summarization (optional)

3. **AgentCoordinator** - Multi-agent coordination
   - Four collab tools: spawn_agent, send_input, wait, close_agent
   - Agent lifecycle: PendingInit → Running → Completed/Errored/Shutdown
   - Multi-turn communication between agents
   - Resource guards (limit concurrent agents)

### Verification
```bash
cargo check -p cocode-executor --manifest-path cocode-rs/Cargo.toml
# Test iterative execution
cargo test -p cocode-executor --manifest-path cocode-rs/Cargo.toml -- iterative
# Test agent coordination
cargo test -p cocode-executor --manifest-path cocode-rs/Cargo.toml -- coordinator
```

## Phase 5: MCP

### Goals
- MCP protocol types
- MCP client for external servers
- MCP server for IDE integration

### Tasks

#### 5.1 Create cocode-mcp-types crate
```
mcp/types/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── protocol.rs     # InitializeRequest, CallToolRequest, etc.
    ├── content.rs      # ContentBlock, TextContent, etc.
    ├── tool.rs         # McpTool, ToolsCapability
    └── notifications.rs # ToolListChanged, Progress, etc.
```

**Key types:**
- `InitializeRequest` / `InitializeResult`
- `CallToolRequest` / `CallToolResult`
- `ListToolsRequest` / `ListToolsResult`
- `McpTool`, `ContentBlock`, `ServerCapabilities`

#### 5.2 Create cocode-mcp-client crate
```
mcp/client/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── transport.rs    # StdioTransport, SseTransport, etc.
    ├── client.rs       # McpClient
    ├── manager.rs      # McpConnectionManager
    ├── auth.rs         # OAuth, bearer token
    └── tool_naming.rs  # mcp__server__tool convention
```

**Key types:**
- `McpTransport` enum (Stdio, Sse, Http, WebSocket)
- `McpClient` - client wrapper
- `McpConnectionManager` - manages multiple connections
- `ToolWithServer` - tool with server qualification

**Integration:**
- Tool naming: `mcp__<server>__<tool>`
- Event emission: `McpToolCallBegin`, `McpToolCallEnd`
- Sandbox state notification to MCP servers

#### 5.3 Create cocode-mcp-server crate
```
mcp/server/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── main.rs         # Entry point
    ├── processor.rs    # MessageProcessor
    ├── handlers.rs     # Request handlers
    └── tools.rs        # cocode, cocode-reply tools
```

**Architecture:** Three-task pattern
- Task 1: stdin reader
- Task 2: message processor
- Task 3: stdout writer

**Exported tools:**
- `cocode` - Start new conversation
- `cocode-reply` - Continue conversation

### Verification
```bash
cargo check -p cocode-mcp-types --manifest-path cocode-rs/Cargo.toml
cargo check -p cocode-mcp-client --manifest-path cocode-rs/Cargo.toml
cargo check -p cocode-mcp-server --manifest-path cocode-rs/Cargo.toml

# Test MCP client with mock server
cargo test -p cocode-mcp-client --manifest-path cocode-rs/Cargo.toml

# Test MCP server
cargo test -p cocode-mcp-server --manifest-path cocode-rs/Cargo.toml
```

## Phase 6: Application

### Goals
- Session management
- CLI entry point
- End-to-end integration

### Tasks

#### 6.1 Create cocode-session crate
```
app/session/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── session.rs      # Session
    ├── config.rs       # SessionConfig
    └── manager.rs      # SessionManager
```

#### 6.2 Create cocode-cli crate
```
app/cli/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── args.rs         # CLI arguments
    └── run.rs          # Main run loop
```

### Verification
```bash
# Build CLI binary
cargo build -p cocode-cli --manifest-path cocode-rs/Cargo.toml

# Integration test
cargo test --manifest-path cocode-rs/Cargo.toml --all-features

# Manual test
./target/debug/cocode-cli "Hello, world!"
```

## Testing Strategy

### Unit Tests
- Each crate has its own unit tests
- Mock LLM responses for loop testing
- Test tool execution in isolation

### Integration Tests
```
cocode-rs/tests/
├── loop_integration.rs    # Full loop with mock model
├── tool_execution.rs      # Tool execution tests
├── subagent_spawn.rs      # Subagent spawning tests
└── e2e/                   # End-to-end tests
    └── basic_conversation.rs
```

### Test Fixtures
```
cocode-rs/tests/fixtures/
├── mock_responses/        # Mock LLM responses
├── test_files/            # Files for Read/Write tests
└── expected_outputs/      # Expected tool outputs
```

## Milestones

### Milestone 1: Foundation Complete
- [ ] cocode-message compiles and tests pass
- [ ] cocode-tools compiles and tests pass
- [ ] cocode-context compiles and tests pass
- [ ] cocode-skill compiles and tests pass
- [ ] Skill validation (field lengths) works
- [ ] Skill scanning (depth limits, symlink safety) works
- [ ] Fail-open skill loading (SkillLoadOutcome) works
- [ ] Bundled skills install with fingerprint detection
- [ ] Integration with hyper-sdk verified

### Milestone 2: Core Loop Working
- [ ] cocode-prompt compiles
- [ ] cocode-loop compiles
- [ ] Can execute single turn with mock model
- [ ] Can execute multi-turn with tool calls

### Milestone 3: Tools Functional
- [ ] All file tools working (Read, Write, Edit, Glob, Grep)
- [ ] Bash tool working with sandboxing
- [ ] Web tools working (WebFetch, WebSearch)

### Milestone 4: Multi-Agent Operational
- [ ] SubagentManager functional
- [ ] 4 built-in agents defined (Bash, general-purpose, Explore, Plan)
- [ ] Task tool spawns subagents with tools[]/disallowed_tools[] filtering
- [ ] Background agents work (both subagent and bash modes)
- [ ] Ctrl+B transition for foreground→background

### Milestone 4.5: Advanced Execution Modes
- [ ] AgentExecutor runs independent full-featured agents
- [ ] IterativeExecutor supports Count and Duration conditions
- [ ] Context passing via prompt injection works
- [ ] Continue-on-error semantics implemented
- [ ] AgentCoordinator manages multiple concurrent agents
- [ ] Collab tools functional (spawn_agent, send_input, wait, close_agent)
- [ ] Agent lifecycle properly tracked

### Milestone 5: MCP Integration
- [ ] cocode-mcp-types compiles
- [ ] cocode-mcp-client connects to stdio MCP server
- [ ] MCP tools discoverable and callable
- [ ] cocode-mcp-server responds to tools/list and tools/call

### Milestone 6: Ready for Use
- [ ] CLI binary builds and runs
- [ ] Full conversation flow works
- [ ] MCP servers connectable
- [ ] All tests pass
- [ ] Documentation complete

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| hyper-sdk API changes | Pin version, abstract interfaces |
| Complex parallel execution | Extensive testing, fallback to sequential |
| Platform-specific issues | Abstract platform code, CI on multiple OS |
| Performance issues | Profile early, optimize hot paths |

## Dependencies

### External (already in workspace)
- `tokio` - Async runtime
- `futures` - Async utilities
- `serde` / `serde_json` - Serialization
- `uuid` - Unique IDs
- `async-trait` - Async traits

### External (new for Phase 1)
- `include_dir` - Embed bundled skills at build time
- `sha2` - Fingerprint computation for bundled skills
- `toml` - SKILL.toml parsing for interface metadata

### Internal (existing)
- `hyper-sdk` - LLM provider abstraction
- `cocode-error` - Error handling
- `cocode-config` - Configuration
- `cocode-utils-pty` - PTY support
