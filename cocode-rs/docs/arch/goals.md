# Project Goals

## Primary Objectives

### 1. Claude Code Alignment

Implement a Rust version that maximizes alignment with Claude Code v2.1.7:

- **Agent Loop**: Recursive turn-based execution with streaming
- **Tool System**: Parallel execution with concurrency safety
- **Subagent Framework**: Task tool for spawning agents with different configs
- **Permission Model**: default/plan/bypass modes
- **State Management**: Context tracking, file read state caching

### 2. Multi-Provider Support

Leverage existing cocode-rs provider infrastructure:

- Reuse `hyper-sdk` for LLM API calls
- Support all existing providers (OpenAI, Anthropic, Gemini, Volcengine, Zai)
- Users specify full model IDs (no automatic aliases or model switching)

### 3. Elegant Crate Organization

Design clean, well-organized crates:

- Logical grouping by functionality (single responsibility)
- Clear dependency hierarchy (no circular dependencies)
- Reasonable crate boundaries (not too granular, not monolithic)
- Easy to extend and test

### 4. Multi-Agent Architecture

Support multiple concurrent agents with different configurations:

- Each agent can have different model
- Each agent can have different tool set
- Agents can run in foreground or background
- Context forking for subagents

### 5. MCP (Model Context Protocol) Support

Implement MCP client and server capabilities (reference: codex-rs mcp-*, Claude Code v2.1.7):

- **MCP Client**: Connect to external MCP servers
  - Multiple transport types: stdio, SSE, HTTP, WebSocket
  - Tool discovery via `tools/list` with memoization
  - Tool naming: `mcp__<server>__<tool>`
  - Auto-search mode for context-efficient tool listing
  - OAuth and bearer token authentication

- **MCP Server**: Expose agent as MCP server
  - Three-task architecture: stdin reader, message processor, stdout writer
  - JSON-RPC 2.0 over stdio
  - Expose `cocode` and `cocode-reply` tools

- **Integration Points**:
  - McpConnectionManager for connection lifecycle
  - Tool registration alongside built-in tools
  - Event streaming for MCP tool calls
  - Sandbox state notification to MCP servers

### 6. Extensibility System

Provide hooks, skills, and plugins for customization:

- **Hooks System** (event-driven extensibility)
  - Events: PreToolUse, PostToolUse, SessionStart, SessionEnd, SubagentStart, PreCompact
  - Multiple scopes: Policy, Plugin, Session
  - Hook matching rules for filtering

- **Skill System** (slash commands)
  - Unified skill/slash command system
  - User-invocable via `/command` syntax
  - Sources: Builtin, Bundled, User, Project, Plugin
  - Skill-level hooks configuration

- **Plugin System** (third-party extensions)
  - Plugin manifest format
  - Commands, skills, agents, hooks from plugins
  - Plugin discovery and loading

### 7. UI Extensibility Architecture

Design core to support multiple UI frontends without modification:

- **Event-Driven Architecture**
  - All UI communication via event channels
  - `LoopEvent` for streaming updates
  - Broadcast channels for fan-out to multiple subscribers

- **app-server Pattern** (for IDE integration)
  - Bidirectional JSON-RPC 2.0 over stdio
  - Three-task architecture: stdin reader, message processor, stdout writer
  - Request types: Initialize, TurnStart, TurnInterrupt, ConfigRead, etc.
  - Notification types: TurnStarted, ItemDelta, ApprovalRequested, etc.

- **TUI Considerations**
  - EventBroker for pause/resume of terminal input
  - FrameRequester/FrameScheduler for efficient redraws
  - Clean separation: widget layer → app event bus → core

### 8. Multi-Model Configuration

Support multiple model types for different use cases:

- **Main Model**: Primary chat model for user conversations
- **Fast Model**: Lightweight model for subagents, hooks, optimizations
  - Default: Same provider's smallest model (e.g., Haiku for Anthropic)
  - Configurable per-provider (Haiku, GPT-4o-mini, Gemini Flash, etc.)
  - Used for: Explore/Bash subagents, prompt hooks, path extraction
- **VLM Model** (Future): Vision-capable model for image processing
  - Separate configuration for multimodal tasks
  - Falls back to main model if it supports vision

**Configuration Schema:**

```toml
# ~/.cocode/config.toml
[models]
# Primary chat model (required)
main = { provider = "anthropic", model = "claude-sonnet-4-20250514" }

# Fast/small model for optimizations (optional)
# If not configured, uses main model for all operations
fast = { provider = "anthropic", model = "claude-haiku-4-5-20250514" }

# Vision model for image processing (optional, future)
# vlm = { provider = "google", model = "gemini-2.0-flash-exp" }
```

**Provider Options for Fast Model:**

| Provider | Fast Model Example | Notes |
|----------|-------------------|-------|
| anthropic | `claude-haiku-4-5-20250514` | Default, lowest cost |
| openai | `gpt-4o-mini` | Fast, affordable |
| google | `gemini-2.0-flash-exp` | Very fast, multimodal |
| volcengine | `doubao-lite-32k` | Fast Chinese model |
| z-ai | `glm-4-flash` | Fast Chinese model |

**Configuration Hierarchy (highest to lowest priority):**

1. Environment variables (`COCODE_FAST_MODEL_PROVIDER`, `COCODE_FAST_MODEL`)
2. Per-session override
3. Config file (`~/.cocode/config.toml`)
4. Provider defaults

**Environment Variables:**

```bash
# Override fast model via environment
COCODE_FAST_MODEL_PROVIDER=google
COCODE_FAST_MODEL=gemini-2.0-flash-exp

# Override vision model via environment (future)
COCODE_VLM_PROVIDER=google
COCODE_VLM_MODEL=gemini-2.0-flash-exp
```

**Use Cases:**

| Use Case | Model Used | Purpose |
|----------|------------|---------|
| User conversation | Main | Primary chat |
| Explore/Bash subagents | Fast | Quick task execution |
| Prompt hooks | Fast | Custom prompt processing |
| Bash output path extraction | Fast | Extract file paths for pre-reading |
| Image processing (future) | VLM | Multimodal tasks |

## Architecture Principles for Extensibility

| Principle | Implementation |
|-----------|----------------|
| Event-driven core | All state changes emit events; UIs subscribe |
| Channel-based IPC | mpsc/broadcast channels between layers |
| Pause/resume support | EventBroker pattern for external process spawning |
| Protocol abstraction | app-server-protocol for IDE/extension communication |
| No UI in core | Core crates have zero UI dependencies |

## Implementation Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Foundation (message, tools, context) | Planned |
| 2 | Core Loop (prompt, loop, agent) | Planned |
| 3 | Execution (sandbox, shell, tools) | Planned |
| 4 | Features (hooks, skill, plugin) | Planned |
| 5 | MCP (mcp-types, mcp-client, mcp-server) | Planned |
| 6 | Application (session, cli) | Planned |
| 7 | TUI (optional, post-MVP) | Deferred |
| 8 | IDE Integration (optional, post-MVP) | Deferred |

## Non-Goals (Initial Version)

- Full TUI implementation (architecture ready, defer UI)
- IDE-specific extensions (architecture ready, defer implementation)
- MCP server-side streaming (basic request/response first)
- Advanced plugin marketplace integration

## Success Criteria

1. Can execute multi-turn conversations with tool calls
2. Can spawn subagents with different tool/model configs
3. Supports parallel tool execution where safe
4. Works with all hyper-sdk providers
5. Clean, maintainable crate structure
6. **MCP client can connect to external servers and invoke tools**
7. **Hooks can intercept and modify tool execution**
8. **Skills can be invoked via `/command` syntax**
9. **Core emits events suitable for multiple UI frontends**
