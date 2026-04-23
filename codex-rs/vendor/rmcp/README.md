<style>
.rustdoc-hidden { display: none; }
</style>

<div class="rustdoc-hidden">

# RMCP: Rust Model Context Protocol

[![Crates.io](https://img.shields.io/crates/v/rmcp.svg)](https://crates.io/crates/rmcp)
[![Documentation](https://docs.rs/rmcp/badge.svg)](https://docs.rs/rmcp)

</div>

`rmcp` is the official Rust implementation of the Model Context Protocol (MCP), a protocol designed for AI assistants to communicate with other services. This library can be used to build both servers that expose capabilities to AI assistants and clients that interact with such servers.

## Quick Start

### Server Implementation

Creating a server with tools is simple using the `#[tool]` macro:

```rust,no_run
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::tool::ToolRouter,
    model::*,
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData as McpError,
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Counter {
    counter: Arc<Mutex<i32>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Counter {
    fn new() -> Self {
        Self {
            counter: Arc::new(Mutex::new(0)),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Increment the counter by 1")]
    async fn increment(&self) -> Result<CallToolResult, McpError> {
        let mut counter = self.counter.lock().await;
        *counter += 1;
        Ok(CallToolResult::success(vec![Content::text(
            counter.to_string(),
        )]))
    }

    #[tool(description = "Get the current counter value")]
    async fn get(&self) -> Result<CallToolResult, McpError> {
        let counter = self.counter.lock().await;
        Ok(CallToolResult::success(vec![Content::text(
            counter.to_string(),
        )]))
    }
}

// Implement the server handler
#[tool_handler]
impl ServerHandler for Counter {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("A simple counter that tallies the number of times the increment tool has been used".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

// Run the server
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create and run the server with STDIO transport
    let service = Counter::new().serve(stdio()).await.inspect_err(|e| {
        println!("Error starting server: {}", e);
    })?;
    service.waiting().await?;
    Ok(())
}
```

### Structured Output

Tools can return structured JSON data with schemas. Use the [`Json`] wrapper:

```rust
# use rmcp::{tool, tool_router, handler::server::{tool::ToolRouter, wrapper::Parameters}, Json};
# use schemars::JsonSchema;
# use serde::{Serialize, Deserialize};
#
#[derive(Serialize, Deserialize, JsonSchema)]
struct CalculationRequest {
    a: i32,
    b: i32,
    operation: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
struct CalculationResult {
    result: i32,
    operation: String,
}

# #[derive(Clone)]
# struct Calculator {
#     tool_router: ToolRouter<Self>,
# }
#
# #[tool_router]
# impl Calculator {
#[tool(name = "calculate", description = "Perform a calculation")]
async fn calculate(&self, params: Parameters<CalculationRequest>) -> Result<Json<CalculationResult>, String> {
    let result = match params.0.operation.as_str() {
        "add" => params.0.a + params.0.b,
        "multiply" => params.0.a * params.0.b,
        _ => return Err("Unknown operation".to_string()),
    };

    Ok(Json(CalculationResult { result, operation: params.0.operation }))
}
# }
```

The `#[tool]` macro automatically generates an output schema from the `CalculationResult` type.

## Tasks

RMCP implements the task lifecycle from SEP-1686 so long-running or asynchronous tool calls can be queued and polled safely.

- **Create:** set the `task` field on `CallToolRequestParam` to ask the server to enqueue the tool call. The response is a `CreateTaskResult` that includes the generated `task.task_id`.
- **Inspect:** use `tasks/get` (`GetTaskInfoRequest`) to retrieve metadata such as status, timestamps, TTL, and poll interval.
- **Await results:** call `tasks/result` (`GetTaskResultRequest`) to block until the task completes and receive either the final `CallToolResult` payload or a protocol error.
- **Cancel:** call `tasks/cancel` (`CancelTaskRequest`) to request termination of a running task.

To expose task support, enable the `tasks` capability when building `ServerCapabilities`. The `#[task_handler]` macro and `OperationProcessor` utility provide reference implementations for enqueuing, tracking, and collecting task results.

### Client Implementation

Creating a client to interact with a server:

```rust,no_run
use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to a server running as a child process
    let service = ()
        .serve(TokioChildProcess::new(Command::new("uvx").configure(
            |cmd| {
                cmd.arg("mcp-server-git");
            },
        ))?)
        .await?;

    // Get server information
    let server_info = service.peer_info();
    println!("Connected to server: {server_info:#?}");

    // List available tools
    let tools = service.list_tools(Default::default()).await?;
    println!("Available tools: {tools:#?}");

    // Call a tool
    let result = service
        .call_tool(CallToolRequestParams {
            meta: None,
            name: "git_status".into(),
            arguments: serde_json::json!({ "repo_path": "." }).as_object().cloned(),
            task: None,
        })
        .await?;
    println!("Result: {result:#?}");

    // Gracefully close the connection
    service.cancel().await?;
    Ok(())
}
```

For more examples, see the [examples directory](https://github.com/anthropics/mcp-rust-sdk/tree/main/examples) in the repository.

## Transport Options

RMCP supports multiple transport mechanisms, each suited for different use cases:

### `transport-async-rw`
Low-level interface for asynchronous read/write operations. This is the foundation for many other transports.

### `transport-io`
For working directly with I/O streams (`tokio::io::AsyncRead` and `tokio::io::AsyncWrite`).

### `transport-child-process`
Run MCP servers as child processes and communicate via standard I/O.

Example:
```rust,ignore
use rmcp::transport::TokioChildProcess;
use tokio::process::Command;

let transport = TokioChildProcess::new(Command::new("mcp-server"))?;
let service = client.serve(transport).await?;
```

## Access with peer interface when handling message

You can get the [`Peer`](crate::service::Peer) struct from [`NotificationContext`](crate::service::NotificationContext) and [`RequestContext`](crate::service::RequestContext).

```rust, ignore
# use rmcp::{
#     ServerHandler,
#     model::{LoggingLevel, LoggingMessageNotificationParam, ProgressNotificationParam},
#     service::{NotificationContext, RoleServer},
# };
# pub struct Handler;

impl ServerHandler for Handler {
    async fn on_progress(
        &self,
        notification: ProgressNotificationParam,
        context: NotificationContext<RoleServer>,
    ) {
        let peer = context.peer;
        let _ = peer
            .notify_logging_message(LoggingMessageNotificationParam {
                level: LoggingLevel::Info,
                logger: None,
                data: serde_json::json!({
                    "message": format!("Progress: {}", notification.progress),
                }),
            })
            .await;
    }
}
```


## Manage Multi Services

For many cases you need to manage several service in a collection, you can call `into_dyn` to convert services into the same type.
```rust, ignore
let service = service.into_dyn();
```

## Feature Flags

RMCP uses feature flags to control which components are included:

- `client`: Enable client functionality
- `server`: Enable server functionality and the tool system
- `macros`: Enable the `#[tool]` macro (enabled by default)
- Transport-specific features:
  - `transport-async-rw`: Async read/write support
  - `transport-io`: I/O stream support
  - `transport-child-process`: Child process support
  - `transport-streamable-http-client` / `transport-streamable-http-server`: HTTP streaming (client agnostic, see [`StreamableHttpClientTransport`](crate::transport::StreamableHttpClientTransport) for details)
    - `transport-streamable-http-client-reqwest`: a default `reqwest` implementation of the streamable http client
- `auth`: OAuth2 authentication support
- `schemars`: JSON Schema generation (for tool definitions)
- TLS backend options (for HTTP transports):
  - `reqwest`: Uses rustls (pure Rust TLS, recommended default)
  - `reqwest-native-tls`: Uses platform native TLS (OpenSSL on Linux, Secure Transport on macOS, SChannel on Windows)
  - `reqwest-tls-no-provider`: Uses rustls without a default crypto provider (bring your own)


## Transports

- `transport-io`: Server stdio transport
- `transport-child-process`: Client stdio transport
- `transport-streamable-http-server` streamable http server transport
- `transport-streamable-http-client` streamable http client transport

<details>
<summary>Transport</summary>

The transport type must implement the [`Transport`](crate::transport::Transport) trait, which allows it to send messages concurrently and receive messages sequentially.
There are 2 pairs of standard transport types:

| transport       | client                                                                              | server                                                                        |
|:---------------:|:-----------------------------------------------------------------------------------:|:-----------------------------------------------------------------------------:|
| std IO          | [`TokioChildProcess`](crate::transport::TokioChildProcess)                          | [`stdio`](crate::transport::stdio)                                            |
| streamable http | [`StreamableHttpClientTransport`](crate::transport::StreamableHttpClientTransport)  | [`StreamableHttpService`](crate::transport::StreamableHttpService)            |

#### [`IntoTransport`](crate::transport::IntoTransport) trait
[`IntoTransport`](crate::transport::IntoTransport) is a helper trait that implicitly converts a type into a transport type.

These types automatically implement [`IntoTransport`](crate::transport::IntoTransport):
1. A type that implements both `futures::Sink` and `futures::Stream`, or a tuple `(Tx, Rx)` where `Tx` is `futures::Sink` and `Rx` is `futures::Stream`.
2. A type that implements both `tokio::io::AsyncRead` and `tokio::io::AsyncWrite`, or a tuple `(R, W)` where `R` is `tokio::io::AsyncRead` and `W` is `tokio::io::AsyncWrite`.
3. A type that implements the [`Worker`](crate::transport::worker::Worker) trait.
4. A type that implements the [`Transport`](crate::transport::Transport) trait.

</details>

## License

This project is licensed under the terms specified in the repository's LICENSE file.
