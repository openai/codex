#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, allow(unused_attributes))]
//! The official Rust SDK for the Model Context Protocol (MCP).
//!
//! The MCP is a protocol that allows AI assistants to communicate with other
//! services. `rmcp` is the official Rust implementation of this protocol.
//!
//! There are two ways in which the library can be used, namely to build a
//! server or to build a client.
//!
//! ## Server
//!
//! A server is a service that exposes capabilities. For example, a common
//! use-case is for the server to make multiple tools available to clients such
//! as Claude Desktop or the Cursor IDE.
//!
//! For example, to implement a server that has a tool that can count, you would
//! make an object for that tool and add an implementation with the `#[tool_router]` macro:
//!
//! ```rust
//! use std::sync::Arc;
//! use rmcp::{ErrorData as McpError, model::*, tool, tool_router, handler::server::tool::ToolRouter};
//! use tokio::sync::Mutex;
//!
//! #[derive(Clone)]
//! pub struct Counter {
//!     counter: Arc<Mutex<i32>>,
//!     tool_router: ToolRouter<Self>,
//! }
//!
//! #[tool_router]
//! impl Counter {
//!     fn new() -> Self {
//!         Self {
//!             counter: Arc::new(Mutex::new(0)),
//!             tool_router: Self::tool_router(),
//!         }
//!     }
//!
//!     #[tool(description = "Increment the counter by 1")]
//!     async fn increment(&self) -> Result<CallToolResult, McpError> {
//!         let mut counter = self.counter.lock().await;
//!         *counter += 1;
//!         Ok(CallToolResult::success(vec![Content::text(
//!             counter.to_string(),
//!         )]))
//!     }
//! }
//! ```
//!
//! ### Structured Output
//!
//! Tools can also return structured JSON data with schemas. Use the [`Json`] wrapper:
//!
//! ```rust
//! # use rmcp::{tool, tool_router, handler::server::{tool::ToolRouter, wrapper::Parameters}, Json};
//! # use schemars::JsonSchema;
//! # use serde::{Serialize, Deserialize};
//! #
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! struct CalculationRequest {
//!     a: i32,
//!     b: i32,
//!     operation: String,
//! }
//!
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! struct CalculationResult {
//!     result: i32,
//!     operation: String,
//! }
//!
//! # #[derive(Clone)]
//! # struct Calculator {
//! #     tool_router: ToolRouter<Self>,
//! # }
//! #
//! # #[tool_router]
//! # impl Calculator {
//! #[tool(name = "calculate", description = "Perform a calculation")]
//! async fn calculate(&self, params: Parameters<CalculationRequest>) -> Result<Json<CalculationResult>, String> {
//!     let result = match params.0.operation.as_str() {
//!         "add" => params.0.a + params.0.b,
//!         "multiply" => params.0.a * params.0.b,
//!         _ => return Err("Unknown operation".to_string()),
//!     };
//!     
//!     Ok(Json(CalculationResult { result, operation: params.0.operation }))
//! }
//! # }
//! ```
//!
//! The `#[tool]` macro automatically generates an output schema from the `CalculationResult` type.
//!
//! Next also implement [ServerHandler] for your server type and start the server inside
//! `main` by calling `.serve(...)`. See the examples directory in the repository for more information.
//!
//! ## Client
//!
//! A client can be used to interact with a server. Clients can be used to get a
//! list of the available tools and to call them. For example, we can `uv` to
//! start a MCP server in Python and then list the tools and call `git status`
//! as follows:
//!
//! ```rust
//! use anyhow::Result;
//! use rmcp::{model::CallToolRequestParam, service::ServiceExt, transport::{TokioChildProcess, ConfigureCommandExt}};
//! use tokio::process::Command;
//!
//! async fn client() -> Result<()> {
//!     let service = ().serve(TokioChildProcess::new(Command::new("uvx").configure(|cmd| {
//!         cmd.arg("mcp-server-git");
//!     }))?).await?;
//!
//!     // Initialize
//!     let server_info = service.peer_info();
//!     println!("Connected to server: {server_info:#?}");
//!
//!     // List tools
//!     let tools = service.list_tools(Default::default()).await?;
//!     println!("Available tools: {tools:#?}");
//!
//!     // Call tool 'git_status' with arguments = {"repo_path": "."}
//!     let tool_result = service
//!         .call_tool(CallToolRequestParam {
//!             name: "git_status".into(),
//!             arguments: serde_json::json!({ "repo_path": "." }).as_object().cloned(),
//!         })
//!         .await?;
//!     println!("Tool result: {tool_result:#?}");
//!
//!     service.cancel().await?;
//!     Ok(())
//! }
//! ```
mod error;
#[allow(deprecated)]
pub use error::{Error, ErrorData, RmcpError};

/// Basic data types in MCP specification
pub mod model;
#[cfg(any(feature = "client", feature = "server"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "client", feature = "server"))))]
pub mod service;
#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use handler::client::ClientHandler;
#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
pub use handler::server::ServerHandler;
#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
pub use handler::server::wrapper::Json;
#[cfg(any(feature = "client", feature = "server"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "client", feature = "server"))))]
pub use service::{Peer, Service, ServiceError, ServiceExt};
#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use service::{RoleClient, serve_client};
#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
pub use service::{RoleServer, serve_server};

pub mod handler;
pub mod transport;

// re-export
#[cfg(all(feature = "macros", feature = "server"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "macros", feature = "server"))))]
pub use paste::paste;
#[cfg(all(feature = "macros", feature = "server"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "macros", feature = "server"))))]
pub use rmcp_macros::*;
#[cfg(all(feature = "macros", feature = "server"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "macros", feature = "server"))))]
pub use schemars;
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use serde;
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use serde_json;
