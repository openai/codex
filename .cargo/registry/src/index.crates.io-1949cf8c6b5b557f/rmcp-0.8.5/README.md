# RMCP: Rust Model Context Protocol

`rmcp` is the official Rust implementation of the Model Context Protocol (MCP), a protocol designed for AI assistants to communicate with other services. This library can be used to build both servers that expose capabilities to AI assistants and clients that interact with such servers.

wait for the first release.
<!-- [![Crates.io](todo)](todo)
[![Documentation](todo)](todo) -->



## Quick Start

### Server Implementation

Creating a server with tools is simple using the `#[tool]` macro:

```rust, ignore
use rmcp::{
    handler::server::router::tool::ToolRouter, model::*, tool, tool_handler, tool_router,
    transport::stdio, ErrorData as McpError, ServiceExt,
};
use std::future::Future;
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
impl rmcp::ServerHandler for Counter {
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

### Client Implementation

Creating a client to interact with a server:

```rust, ignore
use rmcp::{
    model::CallToolRequestParam,
    service::ServiceExt,
    transport::{TokioChildProcess, ConfigureCommandExt}
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
        .call_tool(CallToolRequestParam {
            name: "increment".into(),
            arguments: None,
        })
        .await?;
    println!("Result: {result:#?}");

    // Gracefully close the connection
    service.cancel().await?;
    
    Ok(())
}
```

## Transport Options

RMCP supports multiple transport mechanisms, each suited for different use cases:

### `transport-async-rw`
Low-level interface for asynchronous read/write operations. This is the foundation for many other transports.

### `transport-io`
For working directly with I/O streams (`tokio::io::AsyncRead` and `tokio::io::AsyncWrite`).

### `transport-child-process`
Run MCP servers as child processes and communicate via standard I/O.

Example:
```rust
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
  - `transport-sse-client` / `transport-sse-server`: SSE support (client agnostic)
    - `transport-sse-client-reqwest`: a default `reqwest` implementation of the SSE client
  - `transport-streamable-http-client` / `transport-streamable-http-server`: HTTP streaming (client agnostic, see [`StreamableHttpClientTransport`] for details)
    - `transport-streamable-http-client-reqwest`: a default `reqwest` implementation of the streamable http client
- `auth`: OAuth2 authentication support
- `schemars`: JSON Schema generation (for tool definitions)


## Transports

- `transport-io`: Server stdio transport
- `transport-sse-server`: Server SSE transport
- `transport-child-process`: Client stdio transport
- `transport-sse-client`: Client sse transport
- `transport-streamable-http-server` streamable http server transport
- `transport-streamable-http-client` streamable http client transport

<details>
<summary>Transport</summary>
The transport type must implemented [`Transport`] trait, which allow it send message concurrently and receive message sequentially.
There are 3 pairs of standard transport types:

| transport         | client                                                    | server                                                |
|:-:                |:-:                                                        |:-:                                                    |
| std IO            | [`child_process::TokioChildProcess`]                      | [`io::stdio`]                                         |
| streamable http   | [`streamable_http_client::StreamableHttpClientTransport`] | [`streamable_http_server::session::create_session`]   |
| sse               | [`sse_client::SseClientTransport`]                        | [`sse_server::SseServer`]                             |

#### [IntoTransport](`IntoTransport`) trait
[`IntoTransport`] is a helper trait that implicitly convert a type into a transport type.

These types is automatically implemented [`IntoTransport`] trait
1. A type that already implement both [`futures::Sink`] and [`futures::Stream`] trait, or a tuple `(Tx, Rx)`  where `Tx` is [`futures::Sink`] and `Rx` is [`futures::Stream`].
2. A type that implement both [`tokio::io::AsyncRead`] and [`tokio::io::AsyncWrite`] trait. or a tuple `(R, W)` where `R` is [`tokio::io::AsyncRead`] and `W` is [`tokio::io::AsyncWrite`].
3. A type that implement [Worker](`worker::Worker`) trait.
4. A type that implement [`Transport`] trait.

</details>

## License

This project is licensed under the terms specified in the repository's LICENSE file. 