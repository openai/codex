# MCP (Model Context Protocol) Architecture

## Overview

MCP enables Claude to connect to external tools and services, and allows external applications to interact with the agent.

Reference implementations:
- **codex-rs**: mcp-types, mcp-server, rmcp-client, core/src/mcp/
- **Claude Code v2.1.7**: packages/integrations/src/mcp/

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Agent Loop                                │
│                                                                  │
│  Tool call: "mcp__weather__get_forecast"                        │
│       │                                                          │
│       ▼                                                          │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │              MCP Tool Handler                              │  │
│  │  - Parse tool name → (server, tool)                       │  │
│  │  - Route to McpConnectionManager                          │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  McpConnectionManager                            │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ McpClient   │  │ McpClient   │  │ McpClient   │             │
│  │ (stdio)     │  │ (sse)       │  │ (http)      │             │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │
│         │                │                │                      │
└─────────┼────────────────┼────────────────┼─────────────────────┘
          │                │                │
          ▼                ▼                ▼
    ┌──────────┐     ┌──────────┐     ┌──────────┐
    │External  │     │External  │     │External  │
    │MCP Server│     │MCP Server│     │MCP Server│
    └──────────┘     └──────────┘     └──────────┘
```

## MCP Client

### Transport Types

| Transport | Use Case | Communication |
|-----------|----------|---------------|
| **stdio** | Local CLI servers | Child process stdin/stdout |
| **sse** | Remote streaming | Server-Sent Events |
| **http** | HTTP polling | Request/Response |
| **ws** | Real-time | WebSocket full-duplex |

### Tool Naming Convention

MCP tools use qualified names to prevent conflicts:

```
Format: mcp__<server>__<tool>

Examples:
  mcp__filesystem__read_file
  mcp__github__create_issue
  mcp__weather__get_forecast
```

### Connection Lifecycle

```
1. Configuration Load
   ├─ Read from multiple scopes (priority order)
   └─ Merge configurations

2. Connection Establishment
   ├─ Create transport (stdio/sse/http/ws)
   ├─ Set connection timeout (default: 60s)
   └─ Send initialize request

3. Capability Negotiation
   ├─ Client sends ClientCapabilities
   └─ Server responds with ServerCapabilities

4. Tool Discovery
   ├─ Call tools/list
   ├─ Apply tool filters (enabled/disabled)
   ├─ Cache tool definitions
   └─ Handle pagination (next_cursor)

5. Tool Execution
   ├─ Route qualified tool name
   ├─ Call tools/call with arguments
   ├─ Handle timeout (configurable)
   └─ Return result to agent loop

6. Notification Handling
   ├─ tools/list_changed → refresh cache
   └─ sandbox-state/update → notify servers
```

### Configuration Scopes

| Priority | Scope | Location | Purpose |
|----------|-------|----------|---------|
| 1 | Enterprise | System policy | Organization control |
| 2 | Local | `.cocode.local.toml` | Local overrides |
| 3 | Project | `.mcp.toml` | Project config |
| 4 | User | `~/.config/cocode/mcp.toml` | User defaults |
| 5 | Plugin | Plugin manifests | Plugin-provided |

**Enterprise Override Rule**: When enterprise config exists, it completely overrides all other scopes.

### Configuration Format

```toml
[mcp_servers.weather]
transport = "stdio"
program = "python"
args = ["-m", "weather_server"]
env = { API_KEY = "..." }
enabled = true
startup_timeout_sec = 10
tool_timeout_sec = 60
enabled_tools = ["get_forecast"]  # Optional whitelist
disabled_tools = []               # Optional blacklist

[mcp_servers.api_server]
transport = "sse"
url = "https://api.example.com/mcp"
headers = { Authorization = "Bearer ..." }
# OR for OAuth:
# bearer_token_env_var = "API_TOKEN"
```

## MCP Server

### Three-Task Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        MCP Server                                │
│                                                                  │
│  ┌─────────────────┐                                            │
│  │ Task 1: stdin   │  Read JSON-RPC lines                       │
│  │ reader          │───► Deserialize                            │
│  └────────┬────────┘           │                                │
│           │                    ▼                                │
│           │           ┌─────────────────┐                       │
│           └──────────►│ incoming_tx     │                       │
│                       └────────┬────────┘                       │
│                                │                                │
│                                ▼                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Task 2: message processor                                │   │
│  │                                                          │   │
│  │  select! {                                               │   │
│  │    msg = incoming_rx.recv() => process_request(msg)     │   │
│  │    event = session_rx.recv() => handle_session_event()  │   │
│  │  }                                                       │   │
│  └────────────────────────────┬────────────────────────────┘   │
│                               │                                 │
│                               ▼                                 │
│                       ┌─────────────────┐                       │
│                       │ outgoing_tx     │                       │
│                       └────────┬────────┘                       │
│                                │                                │
│                                ▼                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Task 3: stdout writer                                    │   │
│  │                                                          │   │
│  │  Serialize JSON-RPC ──► Write to stdout                 │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### Exported Tools

The MCP server exposes two tools:

| Tool | Purpose | Input | Output |
|------|---------|-------|--------|
| `cocode` | Start new conversation | `{ prompt: string }` | `{ session_id, response }` |
| `cocode-reply` | Continue conversation | `{ session_id, message }` | `{ response }` |

### Supported Methods

| Method | Purpose |
|--------|---------|
| `initialize` | Handshake, capability exchange |
| `ping` | Health check |
| `tools/list` | List available tools |
| `tools/call` | Execute a tool |
| `resources/list` | List resources (future) |
| `prompts/list` | List prompts (future) |

### Capability Flags

```rust
pub struct ServerCapabilities {
    pub tools: ToolsCapability,
    pub resources: Option<ResourcesCapability>,
    pub prompts: Option<PromptsCapability>,
    pub experimental: Option<HashMap<String, Value>>,
}

// Custom capability for sandbox awareness
// "codex/sandbox-state": true
```

## MCP Types

### Core Protocol Types

```rust
/// Initialize handshake
pub struct InitializeRequest;
pub struct InitializeResult {
    pub protocol_version: String,
    pub server_info: ServerInfo,
    pub capabilities: ServerCapabilities,
}

/// Tool invocation
pub struct CallToolRequest;
pub struct CallToolRequestParams {
    pub name: String,
    pub arguments: Option<Value>,
}
pub struct CallToolResult {
    pub content: Vec<ContentBlock>,
    pub is_error: Option<bool>,
    pub structured_content: Option<Value>,
}

/// Tool discovery
pub struct ListToolsRequest;
pub struct ListToolsResult {
    pub tools: Vec<McpTool>,
    pub next_cursor: Option<String>,
}

/// Tool definition
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}
```

### Content Types

```rust
pub enum ContentBlock {
    Text(TextContent),
    Image(ImageContent),
    Audio(AudioContent),
}

pub struct TextContent {
    pub text: String,
}

pub struct ImageContent {
    pub data: String,      // Base64
    pub mime_type: String,
}
```

### Notifications

```rust
pub struct ToolListChangedNotification;
pub struct ProgressNotification {
    pub progress_token: Value,
    pub progress: f64,
    pub total: Option<f64>,
}
pub struct LoggingMessageNotification {
    pub level: LogLevel,
    pub logger: Option<String>,
    pub data: Value,
}
```

## Integration with Agent Loop

### MCP Tool Handler

```rust
pub struct McpToolHandler {
    connection_manager: Arc<McpConnectionManager>,
}

impl ToolHandler for McpToolHandler {
    fn can_handle(&self, name: &str) -> bool {
        name.starts_with("mcp__")
    }

    async fn handle(&self, call: ToolCall, ctx: ToolContext) -> ToolOutput {
        // Parse qualified name
        let (server, tool) = parse_tool_name(&call.name)?;

        // Emit begin event
        ctx.emit(McpToolCallBegin { server, tool }).await;

        // Execute via connection manager
        let result = self.connection_manager
            .call_tool(&server, &tool, call.arguments)
            .await?;

        // Emit end event
        ctx.emit(McpToolCallEnd { server, tool, result }).await;

        // Convert to tool output
        ToolOutput::from(result)
    }
}
```

### Auto-Search Mode (Claude Code v2.1.7)

Instead of including all MCP tool descriptions in the system prompt, use on-demand search when tool list is large.

```rust
/// MCP auto-search configuration
#[derive(Debug, Clone)]
pub struct McpAutoSearchConfig {
    /// Enable auto-search mode (default: true)
    pub enabled: bool,
    /// Context threshold for enabling search (default: 0.10 = 10%)
    pub context_threshold: f32,
    /// Trigger search on tools/list_changed notification
    pub search_on_list_changed: bool,
    /// Minimum context window required (default: 32000)
    pub min_context_window: i32,
}

impl Default for McpAutoSearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            context_threshold: 0.10,  // 10% of context window
            search_on_list_changed: true,
            min_context_window: 32000,
        }
    }
}

/// Model capability for auto-search support check
pub trait ModelCapabilities {
    fn has_capability(&self, cap: Capability) -> bool;
    fn context_window(&self) -> i32;
}

/// Capability enum for model feature detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    ToolCalling,
    Streaming,
    Vision,
    // ... other capabilities
}

/// Decision logic for auto-search mode (capability-based)
pub fn should_use_auto_search(
    config: &McpAutoSearchConfig,
    model_info: &dyn ModelCapabilities,
    total_tool_description_chars: i32,
) -> bool {
    // Check if enabled
    if !config.enabled {
        return false;
    }

    // Capability-based check: requires tool calling support
    if !model_info.has_capability(Capability::ToolCalling) {
        return false;
    }

    // Check minimum context window
    let context_window = model_info.context_window();
    if context_window < config.min_context_window {
        return false;
    }

    // Check threshold: enable if description chars >= 10% of context
    // Formula: threshold = 0.1 × context_window × 2.5 (chars per token estimate)
    let threshold = (config.context_threshold * context_window as f32 * 2.5) as i32;
    total_tool_description_chars >= threshold
}
```

```
Decision Logic (capability-based):
├─ Capability Check
│  └─ Requires: ToolCalling capability
│
├─ Context Window Check
│  └─ Minimum: 32k tokens (configurable via min_context_window)
│
├─ MCPSearch Tool Available?
│  └─ Required for auto-search
│
└─ Threshold Check (auto mode)
   ├─ total_description_chars = sum(all MCP tools)
   ├─ threshold = 0.1 × context_window × 2.5
   └─ Enable if: description_chars >= threshold
```

This prevents MCP tool descriptions from dominating context.

### Tool Discovery Caching

Cache tool lists per server to avoid repeated discovery calls:

```rust
/// MCP tool cache for avoiding repeated discovery
pub struct McpToolCache {
    /// Cached tools per server
    tools: HashMap<String, Vec<ToolDefinition>>,
    /// Last refresh timestamp per server
    last_refresh: HashMap<String, Instant>,
    /// Invalidate on tools/list_changed notification
    pub invalidate_on_list_changed: bool,
    /// Cache TTL (default: 5 minutes)
    pub cache_ttl: Duration,
}

impl McpToolCache {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            last_refresh: HashMap::new(),
            invalidate_on_list_changed: true,
            cache_ttl: Duration::from_secs(300),
        }
    }

    /// Get cached tools or fetch from server
    pub async fn get_tools(
        &mut self,
        server: &str,
        client: &McpClient,
    ) -> Result<Vec<ToolDefinition>, McpError> {
        // Check cache validity
        if let Some(last) = self.last_refresh.get(server) {
            if last.elapsed() < self.cache_ttl {
                if let Some(tools) = self.tools.get(server) {
                    return Ok(tools.clone());
                }
            }
        }

        // Fetch and cache
        let tools = client.list_tools().await?;
        self.tools.insert(server.to_string(), tools.clone());
        self.last_refresh.insert(server.to_string(), Instant::now());

        Ok(tools)
    }

    /// Invalidate cache for a server (on tools/list_changed)
    pub fn invalidate(&mut self, server: &str) {
        self.tools.remove(server);
        self.last_refresh.remove(server);
    }

    /// Invalidate all caches
    pub fn invalidate_all(&mut self) {
        self.tools.clear();
        self.last_refresh.clear();
    }
}
```

### Wildcard Permissions

Support wildcard patterns for MCP tool permissions:

```rust
/// Check if granted permission matches requested tool
pub fn matches_mcp_permission(granted: &str, requested: &str) -> bool {
    // Exact match
    if granted == requested {
        return true;
    }

    // Wildcard match: "mcp__server__*" matches "mcp__server__read"
    if granted.ends_with("__*") {
        let prefix = &granted[..granted.len() - 1];  // Remove "*"
        return requested.starts_with(prefix);
    }

    // Server wildcard: "mcp__*" matches any tool from any server
    if granted == "mcp__*" {
        return requested.starts_with("mcp__");
    }

    false
}

// Examples:
// "mcp__filesystem__*" matches "mcp__filesystem__read", "mcp__filesystem__write"
// "mcp__github__create_*" matches "mcp__github__create_issue", "mcp__github__create_pr"
// "mcp__*" matches any MCP tool
```

## Events

### MCP-Related Loop Events

```rust
pub enum LoopEvent {
    // ... other events ...

    // MCP tool execution
    McpToolCallBegin {
        server: String,
        tool: String,
        call_id: String,
    },
    McpToolCallEnd {
        server: String,
        tool: String,
        call_id: String,
        result: CallToolResult,
        is_error: bool,
    },

    // MCP server startup
    McpStartupUpdate {
        server: String,
        status: McpStartupStatus,
    },
    McpStartupComplete {
        servers: Vec<McpServerInfo>,
        failed: Vec<(String, String)>,
    },
}
```

## Timeouts and Limits

| Parameter | Default | Purpose |
|-----------|---------|---------|
| Connection timeout | 60s | Server startup |
| Tool timeout | 27h* | Individual tool execution |
| Progress interval | 30s | Progress reporting |
| Keep-alive | 50s | Connection heartbeat |
| Max output tokens | 25000 | Tool output truncation |
| Batch size | 3 | Concurrent server connections |

*Effectively unlimited; configurable via `MCP_TOOL_TIMEOUT`

## Server Health Monitoring

Monitor MCP server health with ping/reconnect:

```rust
/// Server health monitoring configuration
#[derive(Debug, Clone)]
pub struct ServerHealthConfig {
    /// Ping interval (default: 30s)
    pub ping_interval: Duration,
    /// Ping timeout (default: 5s)
    pub timeout: Duration,
    /// Reconnect backoff configuration
    pub reconnect_backoff: ExponentialBackoff,
    /// Max reconnect attempts (default: 5)
    pub max_reconnect_attempts: i32,
}

impl Default for ServerHealthConfig {
    fn default() -> Self {
        Self {
            ping_interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            reconnect_backoff: ExponentialBackoff {
                initial_interval: Duration::from_millis(100),
                max_interval: Duration::from_secs(10),
                multiplier: 2.0,
            },
            max_reconnect_attempts: 5,
        }
    }
}

/// Exponential backoff for reconnection
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    pub initial_interval: Duration,
    pub max_interval: Duration,
    pub multiplier: f64,
}

impl McpClient {
    /// Start health monitoring task
    pub fn start_health_monitor(&self, config: ServerHealthConfig) -> JoinHandle<()> {
        let client = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.ping_interval);
            let mut consecutive_failures = 0;

            loop {
                interval.tick().await;

                match tokio::time::timeout(config.timeout, client.ping()).await {
                    Ok(Ok(_)) => {
                        consecutive_failures = 0;
                    }
                    _ => {
                        consecutive_failures += 1;
                        if consecutive_failures >= config.max_reconnect_attempts {
                            // Trigger reconnection
                            client.reconnect_with_backoff(&config.reconnect_backoff).await;
                            consecutive_failures = 0;
                        }
                    }
                }
            }
        })
    }
}
```

## Sandbox State Notification

When sandbox policy changes, notify MCP servers:

```rust
impl McpConnectionManager {
    pub fn notify_sandbox_state_change(&self, state: SandboxState) {
        for client in self.clients.values() {
            if client.supports_capability("codex/sandbox-state") {
                client.send_notification(
                    "codex/sandbox-state/update",
                    state.clone()
                );
            }
        }
    }
}
```

This allows MCP servers to adapt behavior based on sandbox restrictions.

## Error Handling

### Client Errors

| Error | Handling |
|-------|----------|
| Connection failed | Retry with backoff, mark server failed |
| Tool not found | Return error to agent |
| Tool timeout | Cancel, return timeout error |
| Parse error | Return malformed response error |
| Auth required | Trigger OAuth flow (if supported) |

### Server Errors

| Error | Response |
|-------|----------|
| Invalid request | JSON-RPC error -32600 |
| Method not found | JSON-RPC error -32601 |
| Invalid params | JSON-RPC error -32602 |
| Internal error | JSON-RPC error -32603 |

## Crate Structure

```
mcp/
├── types/                  # cocode-mcp-types (~1500 LOC)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── protocol.rs     # InitializeRequest, CallToolRequest, etc.
│       ├── content.rs      # ContentBlock, TextContent, etc.
│       ├── tool.rs         # McpTool, ToolsCapability
│       └── notifications.rs # ToolListChanged, Progress, etc.
│
├── client/                 # cocode-mcp-client (~1200 LOC)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── transport.rs    # StdioTransport, SseTransport, etc.
│       ├── client.rs       # McpClient
│       ├── manager.rs      # McpConnectionManager
│       ├── auth.rs         # OAuth, bearer token
│       └── tool_naming.rs  # mcp__server__tool convention
│
└── server/                 # cocode-mcp-server (~800 LOC)
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── main.rs         # Entry point
        ├── processor.rs    # MessageProcessor
        ├── handlers.rs     # Request handlers
        └── tools.rs        # cocode, cocode-reply tools
```

## Implementation Reference

### codex-rs Files

| File | Purpose |
|------|---------|
| `mcp-types/src/lib.rs` | Protocol types (7000+ LOC, auto-generated) |
| `mcp-server/src/lib.rs` | Server orchestration |
| `mcp-server/src/message_processor.rs` | Request routing |
| `rmcp-client/src/rmcp_client.rs` | Client implementation |
| `rmcp-client/src/oauth.rs` | OAuth token management |
| `core/src/mcp_connection_manager.rs` | Connection pool |
| `core/src/mcp_tool_call.rs` | Tool execution |

### Claude Code v2.1.7 Files

| File | Purpose |
|------|---------|
| `packages/integrations/src/mcp/types.ts` | Type definitions |
| `packages/integrations/src/mcp/config.ts` | Configuration loading |
| `packages/integrations/src/mcp/connection.ts` | Transport management |
| `packages/integrations/src/mcp/discovery.ts` | Tool discovery |
| `packages/integrations/src/mcp/execution.ts` | Tool execution |
| `packages/integrations/src/mcp/autosearch.ts` | Auto-search logic |
