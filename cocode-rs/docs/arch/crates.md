# Crate Organization (Revised)

## Design Principles

1. **Align with Claude Code structure**: core, features, integrations
2. **Agent in core**: Subagent system is tightly coupled with agent loop
3. **Features layer**: Hooks, skills, plugins as separate extensibility crates
4. **Clean dependencies**: No circular dependencies
5. **Reasonable crate boundaries**: Functional grouping, not arbitrary LOC limits

## Directory Structure

```
cocode-rs/
├── common/                     # Existing (4 crates)
│   ├── error/                  # cocode-error
│   ├── protocol/               # cocode-protocol
│   ├── config/                 # cocode-config
│   └── otel/                   # cocode-otel
│
├── core/                       # Core execution engine (7 crates)
│   ├── message/                # cocode-message
│   ├── tools/                  # cocode-tools (trait, registry, built-in implementations)
│   ├── context/                # cocode-context
│   ├── prompt/                 # cocode-prompt
│   ├── loop/                   # cocode-loop
│   ├── subagent/               # cocode-subagent (Task tool, context inheritance)
│   └── executor/               # cocode-executor (AgentExecutor, iterative, collab)
│
├── features/                   # Extensibility features (3 crates)
│   ├── hooks/                  # cocode-hooks
│   ├── skill/                  # cocode-skill (slash commands + skills)
│   └── plugin/                 # cocode-plugin
│
├── mcp/                        # MCP layer (3 crates)
│   ├── types/                  # cocode-mcp-types
│   ├── client/                 # cocode-mcp-client
│   └── server/                 # cocode-mcp-server
│
├── exec/                       # Execution layer (2 crates)
│   ├── sandbox/                # cocode-sandbox
│   └── shell/                  # cocode-shell
│
├── app/                        # Application layer (2 crates)
│   ├── session/                # cocode-session
│   └── cli/                    # cocode-cli
│
├── provider-sdks/              # Existing (6 crates)
│   ├── hyper-sdk/
│   ├── anthropic/
│   ├── openai/
│   ├── google-genai/
│   ├── volcengine-ark/
│   └── z-ai/
│
└── utils/                      # Existing (14 crates)
```

**Total: 17 new crates**

## Core Layer (core/)

### cocode-message

Extended message types building on hyper-sdk.

```rust
pub struct ConversationMessage { ... }
pub struct MessageMetadata { ... }
pub enum AttachmentType { Progress, System, Error }
```

### cocode-tools

Tool trait, registry, and built-in implementations (merged for convenience).

```rust
// Tool abstraction
pub trait Tool: Send + Sync { ... }
pub struct ToolRegistry { ... }
pub struct ToolFilter { ... }
pub struct ToolContext { ... }
pub enum ConcurrencySafety { Safe, Unsafe }

// File tools
pub struct ReadTool;
pub struct WriteTool;
pub struct EditTool;
pub struct GlobTool;
pub struct GrepTool;

// Shell tools
pub struct BashTool;

// Web tools
pub struct WebFetchTool;
pub struct WebSearchTool;

// Agent tools
pub struct TaskTool;          // Spawns subagents
pub struct AskUserQuestionTool;
pub struct EnterPlanModeTool;
pub struct ExitPlanModeTool;

// Task management tools
pub struct TodoWriteTool;     // Atomic replace of the whole todo list

// Background task helpers
pub struct TaskOutputTool;    // Retrieve output from background task
pub struct KillShellTool;     // Kill background bash shell by ID
```

### cocode-context

State and context management.

```rust
pub struct ConversationContext { ... }
pub struct AppState { ... }
pub struct PermissionContext { ... }
pub enum PermissionMode { Default, Plan, Bypass }
```

### cocode-prompt

Prompt building utilities.

```rust
pub struct PromptBuilder { ... }
pub fn build_system_prompt(...) -> String;
pub fn format_system_reminder(content: &str) -> String;
```

### cocode-loop

Main agent loop driver.

```rust
pub struct AgentLoop { ... }
pub struct LoopConfig { ... }
pub enum LoopEvent { ... }
pub struct ToolExecutor { ... }  // Parallel/sequential execution
```

### cocode-subagent

Subagent system for Task tool spawning. Depends on cocode-executor for base AgentExecutor.

```rust
pub struct AgentDefinition { ... }
pub struct SubagentManager { ... }
pub struct SpawnInput { ... }
pub enum ModelSelection { Inherit, Alias(String), Specific { provider, model } }

// Built-in agents (4)
pub fn builtin_agents() -> Vec<AgentDefinition>;  // Bash, general-purpose, Explore, Plan

// Three-layer tool filtering
pub fn filter_tools_for_agent(tools: &[Arc<dyn Tool>], agent_def: &AgentDefinition) -> Vec<Arc<dyn Tool>>;
```

**Key distinction from cocode-executor:**
- **cocode-subagent**: Context inheritance from parent, tool filtering, Task tool integration
- **cocode-executor**: Independent agent execution, no parent context

### cocode-executor

Base execution primitives and advanced execution modes.
See [execution-modes.md](execution-modes.md) for detailed architecture.

```rust
/// Base primitive: independent agent execution (no parent context)
/// This is the foundation used by subagent, iterative, and collab patterns.
pub struct AgentExecutor { ... }
pub struct AgentExecutorConfig { ... }
pub struct AgentResult { ... }

/// Iterative execution (multi-run for same requirement)
/// Entry points: CLI --iter, slash command /iter
pub struct IterativeExecutor { ... }
pub enum IterationCondition {
    Count { count: i32 },      // Run N times: "5"
    Duration { seconds: i64 }, // Run for T duration: "2h"
}
pub struct IterationContext { ... }
pub struct IterationRecord { ... }

/// Multi-agent coordination (collab pattern)
pub struct AgentCoordinator { ... }
pub struct ThreadId(Uuid);
pub enum AgentStatus {
    PendingInit,
    Running,
    Completed(Option<String>),
    Errored(String),
    Shutdown,
    NotFound,
}

// Collab tools
pub struct SpawnAgentTool;    // spawn_agent
pub struct SendInputTool;     // send_input (multi-turn)
pub struct WaitTool;          // wait for completion
pub struct CloseAgentTool;    // close_agent
```

**Dependency: cocode-subagent → cocode-executor → cocode-loop**

## Features Layer (features/)

### cocode-hooks

Event-driven extensibility (modeled after Claude Code hooks).

```rust
/// Hook event types (matching Claude Code)
pub enum HookEventType {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    UserPromptSubmit,
    SessionStart,
    SessionEnd,
    Stop,
    SubagentStart,
    SubagentStop,
    PreCompact,
    PermissionRequest,
}

/// Hook definition
pub struct HookDefinition {
    pub hook_type: HookType,  // command, prompt, agent, callback
    pub matcher: String,       // Tool name pattern (regex or "Write|Read")
    pub once: bool,            // One-shot hook
}

/// Hook types
pub enum HookType {
    Command { command: String, timeout: Option<Duration> },
    Prompt { prompt: String, model: Option<String> },
    Agent { prompt: String, timeout: Option<Duration> },
    Callback(HookCallback),
}

/// Hook registry with multiple scopes
pub struct HookRegistry {
    policy_hooks: HashMap<HookEventType, Vec<HookMatcher>>,
    plugin_hooks: HashMap<HookEventType, Vec<HookMatcher>>,
    session_hooks: HashMap<String, HashMap<HookEventType, Vec<HookMatcher>>>,
}

impl HookRegistry {
    pub fn aggregate_hooks(&self, event: HookEventType, session_id: &str) -> Vec<&HookDefinition>;
    pub fn add_session_hook(&mut self, session_id: &str, event: HookEventType, hook: HookDefinition);
    pub fn remove_session_hook(&mut self, session_id: &str, event: HookEventType, hook: &HookDefinition);
}

/// Hook execution result
pub struct HookResult {
    pub outcome: HookOutcome,  // success, failure, skipped
    pub output: Option<String>,
}
```

### cocode-skill

Unified skill/slash command system.

```rust
/// Skill definition (matches Claude Code's unified model)
pub struct Skill {
    pub name: String,
    pub skill_type: SkillType,  // prompt, local
    pub source: SkillSource,     // plugin, builtin, bundled, user
    pub description: String,
    pub aliases: Vec<String>,
    pub when_to_use: Option<String>,  // LLM guidance
    pub allowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
    pub user_invocable: bool,
    pub disable_model_invocation: bool,
    pub hooks: Option<HooksConfig>,   // Skill-level hooks
}

pub enum SkillType {
    Prompt { content: String },
    Local { handler: Box<dyn SkillHandler> },
}

pub enum SkillSource {
    Builtin,
    Bundled,
    User,
    Project,
    Plugin { plugin_id: String },
}

/// Skill registry with loading from multiple sources
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub async fn load_all(&mut self, ctx: &LoadContext) -> Result<()>;
    pub fn get_llm_invocable(&self) -> Vec<&Skill>;
    pub fn get_user_invocable(&self) -> Vec<&Skill>;
    pub async fn execute(&self, name: &str, args: &str, ctx: &ExecutionContext) -> Result<SkillOutput>;
}

/// Skill loading directories (priority order)
/// 1. Managed: ~/.claude/skills/ (policy)
/// 2. User: <user-config>/.claude/skills/
/// 3. Project: ./.claude/skills/
```

### cocode-plugin

Plugin system for extensibility.

```rust
/// Plugin definition
pub struct Plugin {
    pub id: PluginId,  // name@marketplace
    pub name: String,
    pub manifest: PluginManifest,
    pub path: PathBuf,
    pub enabled: bool,
    pub scope: PluginScope,
}

pub enum PluginScope {
    Managed,   // Enterprise/policy
    User,      // User-level
    Project,   // Project-specific
    Inline,    // --plugin-dir
}

/// Plugin manifest (plugin.json)
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub commands: Option<Vec<String>>,  // Command paths
    pub skills: Option<Vec<String>>,     // Skill paths
    pub agents: Option<Vec<AgentDefinition>>,
    pub hooks: Option<HooksConfig>,
}

/// Plugin manager
pub struct PluginManager {
    plugins: HashMap<PluginId, Plugin>,
    marketplaces: HashMap<String, Marketplace>,
}

impl PluginManager {
    pub async fn discover(&mut self) -> Result<()>;
    pub async fn install(&mut self, plugin_id: &str) -> Result<()>;
    pub fn enable(&mut self, plugin_id: &str) -> Result<()>;
    pub fn disable(&mut self, plugin_id: &str) -> Result<()>;
    pub fn get_commands(&self) -> Vec<&Skill>;
    pub fn get_agents(&self) -> Vec<&AgentDefinition>;
    pub fn get_hooks(&self) -> HooksConfig;
}
```

## MCP Layer (mcp/)

Reference: codex-rs mcp-*, Claude Code v2.1.7 MCP implementation.

### cocode-mcp-types

MCP protocol type definitions (spec-compliant).

```rust
/// Core protocol types (auto-generated or manual from spec)
pub struct InitializeRequest { ... }
pub struct InitializeResult { ... }

pub struct CallToolRequest { ... }
pub struct CallToolResult {
    pub content: Vec<ContentBlock>,
    pub is_error: Option<bool>,
}

pub struct ListToolsRequest { ... }
pub struct ListToolsResult {
    pub tools: Vec<Tool>,
    pub next_cursor: Option<String>,
}

/// Notification types
pub struct ToolListChangedNotification;
pub struct ProgressNotification { ... }

/// Content types
pub enum ContentBlock {
    Text(TextContent),
    Image(ImageContent),
    Audio(AudioContent),
}

/// Tool definition
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}
```

### cocode-mcp-client

MCP client for connecting to external servers.

```rust
/// Transport types supported
pub enum McpTransport {
    Stdio { program: String, args: Vec<String>, env: HashMap<String, String> },
    Sse { url: String, headers: HashMap<String, String> },
    Http { url: String, headers: HashMap<String, String> },
    WebSocket { url: String, headers: HashMap<String, String> },
}

/// MCP client wrapper
pub struct McpClient {
    transport: Box<dyn Transport>,
    capabilities: ServerCapabilities,
    server_info: ServerInfo,
}

impl McpClient {
    pub async fn connect(config: McpServerConfig) -> Result<Self>;
    pub async fn initialize(&self) -> Result<InitializeResult>;
    pub async fn list_tools(&self) -> Result<ListToolsResult>;
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<CallToolResult>;
    pub async fn list_resources(&self) -> Result<ListResourcesResult>;
    pub async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult>;
}

/// Connection manager for multiple MCP servers
pub struct McpConnectionManager {
    clients: HashMap<String, AsyncManagedClient>,
    tool_cache: RwLock<HashMap<String, Vec<McpTool>>>,
}

impl McpConnectionManager {
    pub async fn initialize(&mut self, configs: Vec<McpServerConfig>) -> Result<()>;
    pub async fn list_all_tools(&self) -> Result<Vec<ToolWithServer>>;
    pub async fn call_tool(&self, server: &str, tool: &str, args: Value) -> Result<CallToolResult>;
    pub fn notify_sandbox_state_change(&self, state: SandboxState);
}

/// Tool naming convention: mcp__<server>__<tool>
pub fn make_tool_name(server: &str, tool: &str) -> String;
pub fn parse_tool_name(name: &str) -> Option<(String, String)>;
```

### cocode-mcp-server

MCP server exposing agent as MCP endpoint.

```rust
/// MCP server (three-task architecture)
pub struct McpServer {
    agent_session: Arc<Session>,
    running_sessions: HashMap<String, RunningConversation>,
}

impl McpServer {
    pub async fn run(session: Arc<Session>) -> Result<()>;
}

/// Exported tools
/// - "cocode": Start new conversation
/// - "cocode-reply": Send message to existing conversation

/// Internal task structure
/// Task 1: stdin_reader - reads JSON-RPC from stdin
/// Task 2: message_processor - routes to handlers
/// Task 3: stdout_writer - writes responses to stdout

/// Message processor
pub struct MessageProcessor {
    handlers: HashMap<String, Box<dyn RequestHandler>>,
}

impl MessageProcessor {
    pub fn register_handler(&mut self, method: &str, handler: impl RequestHandler);
    pub async fn process(&self, request: JsonRpcRequest) -> JsonRpcResponse;
}

/// Supported methods
/// - initialize
/// - ping
/// - tools/list
/// - tools/call
/// - resources/list (future)
/// - prompts/list (future)
```

## Execution Layer (exec/)

### cocode-sandbox

Sandboxing and permission enforcement.

```rust
pub struct Sandbox { ... }
pub struct SandboxConfig { ... }
pub enum SandboxMode { None, Basic, Strict }
```

### cocode-shell

Command execution.

```rust
pub struct ShellExecutor { ... }
pub struct CommandResult { ... }
pub fn is_read_only_command(cmd: &str) -> bool;
```

## Application Layer (app/)

### cocode-session

Session management.

```rust
pub struct Session { ... }
pub struct SessionConfig { ... }
```

### cocode-cli

CLI entry point.

```rust
pub fn main() -> Result<()>;
pub struct CliArgs { ... }
```

## Dependency Graph

```
                                ┌─────────────────┐
                                │   cocode-cli    │
                                └────────┬────────┘
                                         │
                                ┌────────▼────────┐
                                │ cocode-session  │
                                └────────┬────────┘
                                         │
           ┌─────────────────────────────┼─────────────────────────────┐
           │                             │                             │
  ┌────────▼────────┐          ┌────────▼────────┐          ┌─────────▼─────────┐
  │  cocode-plugin  │          │  cocode-skill   │          │   cocode-hooks    │
  └────────┬────────┘          └────────┬────────┘          └─────────┬─────────┘
           │                             │                             │
           └─────────────────────────────┼─────────────────────────────┘
                                         │
                  ┌──────────────────────┼──────────────────────────────┐
                  │                      │                              │
         ┌────────▼────────┐   ┌────────▼────────┐            ┌────────▼────────┐
         │cocode-subagent  │   │  cocode-tools   │            │  cocode-shell   │
         └────────┬────────┘   └────────┬────────┘            └────────┬────────┘
                  │
         ┌────────▼────────┐
         │cocode-executor  │
         └────────┬────────┘
                  │                      │                              │
                  └──────────┬───────────┘                              │
                             │                                          │
                    ┌────────▼────────┐                        ┌────────▼────────┐
                    │   cocode-loop   │                        │ cocode-sandbox  │
                    └────────┬────────┘                        └─────────────────┘
                             │
            ┌────────────────┼────────────────┐
            │                │                │
   ┌────────▼────────┐ ┌─────▼──────┐ ┌──────▼────────┐
   │  cocode-prompt  │ │cocode-     │ │cocode-message │
   │                 │ │context     │ │               │
   └────────┬────────┘ └─────┬──────┘ └───────┬───────┘
            │                │                │
            └────────────────┼────────────────┘
                             │
      ┌──────────────────────┼──────────────────────────────────────────────┐
      │                      │                                              │
   ┌──▼───────────┐  ┌───────▼───────┐  ┌──────────────┐  ┌────────────────▼┐
   │  hyper-sdk   │  │cocode-protocol│  │ cocode-config│  │   cocode-error  │
   └──────────────┘  └───────────────┘  └──────────────┘  └─────────────────┘
```

## Crate Summary

| Layer | Crate | Purpose |
|-------|-------|---------|
| Core | cocode-message | Message types |
| Core | cocode-tools | Tool trait, registry, built-in implementations |
| Core | cocode-context | State management |
| Core | cocode-prompt | Prompt building |
| Core | cocode-loop | Agent loop driver |
| Core | cocode-subagent | Subagent system (Task tool, context inheritance) |
| Core | cocode-executor | Base AgentExecutor + advanced execution (iterative, collab) |
| Features | cocode-hooks | Event-driven hooks |
| Features | cocode-skill | Slash commands + skills |
| Features | cocode-plugin | Plugin system |
| MCP | cocode-mcp-types | MCP protocol types |
| MCP | cocode-mcp-client | MCP client + connection manager |
| MCP | cocode-mcp-server | MCP server (3-task architecture) |
| Exec | cocode-sandbox | Sandboxing |
| Exec | cocode-shell | Command execution |
| App | cocode-session | Session management |
| App | cocode-cli | CLI entry |

**Total: 17 new crates**

## Cargo.toml Updates

```toml
[workspace]
members = [
    # ... existing members ...

    # Core
    "core/message",
    "core/tools",
    "core/context",
    "core/prompt",
    "core/loop",
    "core/subagent",
    "core/executor",

    # Features
    "features/hooks",
    "features/skill",
    "features/plugin",

    # MCP
    "mcp/types",
    "mcp/client",
    "mcp/server",

    # Exec
    "exec/sandbox",
    "exec/shell",

    # App
    "app/session",
    "app/cli",
]

[workspace.dependencies]
# Internal - core
cocode-message = { path = "core/message" }
cocode-tools = { path = "core/tools" }
cocode-context = { path = "core/context" }
cocode-prompt = { path = "core/prompt" }
cocode-loop = { path = "core/loop" }
cocode-subagent = { path = "core/subagent" }
cocode-executor = { path = "core/executor" }

# Internal - features
cocode-hooks = { path = "features/hooks" }
cocode-skill = { path = "features/skill" }
cocode-plugin = { path = "features/plugin" }

# Internal - mcp
cocode-mcp-types = { path = "mcp/types" }
cocode-mcp-client = { path = "mcp/client" }
cocode-mcp-server = { path = "mcp/server" }

# Internal - exec
cocode-sandbox = { path = "exec/sandbox" }
cocode-shell = { path = "exec/shell" }

# Internal - app
cocode-session = { path = "app/session" }
cocode-cli = { path = "app/cli" }
```

## Implementation Phases (Revised)

| Phase | Focus | Crates |
|-------|-------|--------|
| 1 | Foundation | message, tools, context |
| 2 | Core Loop | prompt, loop, agent |
| 3 | Execution | sandbox, shell |
| 4 | Features | hooks, skill, plugin |
| 4.5 | Advanced Execution | executor (iterative, collab) |
| 5 | MCP | mcp-types, mcp-client, mcp-server |
| 6 | Application | session, cli, integration |

## Comparison with Claude Code

| Claude Code Package | cocode-rs Crate(s) |
|---------------------|-------------------|
| packages/core | core/loop, core/tools, core/context |
| packages/tools | core/tools |
| packages/features | features/hooks, features/skill |
| packages/plugin | features/plugin |
| packages/integrations | core/subagent, core/executor, mcp/client, mcp/server |
| packages/integrations/mcp | mcp/types, mcp/client |
| packages/platform | exec/sandbox, exec/shell |
| packages/shared | common/, core/message, mcp/types |
| packages/cli | app/cli, app/session |

### Comparison with codex-rs MCP

| codex-rs Crate | cocode-rs Crate |
|----------------|-----------------|
| mcp-types | mcp/types (cocode-mcp-types) |
| mcp-server | mcp/server (cocode-mcp-server) |
| rmcp-client | mcp/client (cocode-mcp-client) |
| core/src/mcp_connection_manager | mcp/client (McpConnectionManager) |
| core/src/mcp_tool_call | core/tools (MCP tool handler) |

## Key Features Coverage

### Slash Commands (cocode-skill)
- **Location**: `features/skill/`
- Unified skill/slash command system (like Claude Code since v2.1.3)
- `user_invocable: bool` flag enables `/command` invocation
- Loading priority: Managed → User → Project → Plugin
- Examples: `/commit`, `/review-pr`, `/help`

### Plan Mode (cocode-tools + cocode-context)
- **Location**: `core/tools/` and `core/context/`
- `EnterPlanModeTool` - Transitions agent to plan mode
- `ExitPlanModeTool` - Exits plan mode with user approval
- `PermissionMode::Plan` - Read-only exploration mode
- Plan files stored in `~/.claude/plans/` with unique slugs
- 5-phase workflow: Understanding → Design → Review → Final Plan → Exit

### Context Compaction (cocode-loop)
- **Location**: `core/loop/`
- Auto-compaction when context exceeds threshold (default 0.8)
- `should_compact()` checks token usage vs context window
- `compact()` summarizes older messages
- `PreCompact` hook event for extensibility
- Events: `CompactionStarted`, `CompactionCompleted`

## Architecture Review Notes

### Strengths
1. **Clean layering**: common → core → features → exec → app
2. **Reasonable crate boundaries**: Functional grouping, single responsibility
3. **No circular dependencies**: Unidirectional dependency flow
4. **Reuses existing infrastructure**: hyper-sdk, cocode-error, cocode-config

### Trade-offs Considered

| Decision | Alternative | Rationale |
|----------|-------------|-----------|
| agent in core/ | Separate agent/ layer | Tight coupling with loop justifies core placement |
| tools merged (trait + impl) | Split into tool/tools | Single crate simpler; trait and implementations closely related |
| prompt separate from loop | Merge into loop | Allows prompt reuse in CLI/session; clearer responsibility |
| features/ as separate layer | Inline in core | Extensibility deserves explicit isolation |

### Dependency Considerations

The features layer requires careful dependency management:
```
cocode-hooks ← cocode-skill ← cocode-plugin
     ↑              ↑              ↑
     └──────────────┴──────────────┘
              shared types
```

To avoid circular dependencies:
- `cocode-hooks` has no dependencies on skill/plugin
- `cocode-skill` depends on hooks (skills can have hooks)
- `cocode-plugin` depends on both (plugins provide all)
