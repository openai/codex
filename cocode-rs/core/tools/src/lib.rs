//! cocode-tools - Tool execution layer for the agent system.
//!
//! This crate provides the tool system for the agent:
//! - Tool trait with 5-stage pipeline and input-dependent concurrency
//! - Tool registry (built-in + MCP)
//! - Streaming tool executor
//! - 16 built-in tools aligned with Claude Code v2.1.7
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        cocode-tools                             │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Tool Trait          │  ToolRegistry       │  Executor          │
//! │  - validate()        │  - builtin tools    │  - concurrent exec │
//! │  - check_permission()│  - MCP tools        │  - sequential exec │
//! │  - execute()         │  - aliases          │  - abort handling  │
//! │  - post_process()    │                     │                    │
//! │  - cleanup()         │                     │                    │
//! ├──────────────────────┴─────────────────────┴────────────────────┤
//! │  Built-in Tools (16): Read, Glob, Grep, Edit, Write, Bash,     │
//! │  Task, TaskOutput, KillShell, TodoWrite, EnterPlanMode,         │
//! │  ExitPlanMode, AskUserQuestion, WebFetch, WebSearch, Skill      │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Quick Start
//!
//! ```ignore
//! use cocode_tools::{ToolRegistry, StreamingToolExecutor, ExecutorConfig};
//! use cocode_tools::builtin::register_builtin_tools;
//! use std::sync::Arc;
//!
//! // Create and populate registry
//! let mut registry = ToolRegistry::new();
//! register_builtin_tools(&mut registry);
//!
//! // Create executor
//! let config = ExecutorConfig::default();
//! let executor = StreamingToolExecutor::new(Arc::new(registry), config, None);
//!
//! // During streaming - when tool_use block completes
//! executor.on_tool_complete(tool_call).await;
//!
//! // After message_stop - execute pending unsafe tools
//! executor.execute_pending_unsafe().await;
//!
//! // Get all results
//! let results = executor.drain().await;
//! ```
//!
//! # Implementing Custom Tools
//!
//! ```ignore
//! use cocode_tools::{Tool, ToolContext, ToolOutput, ToolError};
//! use async_trait::async_trait;
//!
//! struct MyTool;
//!
//! #[async_trait]
//! impl Tool for MyTool {
//!     fn name(&self) -> &str { "my_tool" }
//!     fn description(&self) -> &str { "My custom tool" }
//!     fn input_schema(&self) -> serde_json::Value {
//!         serde_json::json!({
//!             "type": "object",
//!             "properties": {
//!                 "input": {"type": "string"}
//!             },
//!             "required": ["input"]
//!         })
//!     }
//!
//!     async fn execute(
//!         &self,
//!         input: serde_json::Value,
//!         ctx: &mut ToolContext,
//!     ) -> Result<ToolOutput, ToolError> {
//!         let value = input["input"].as_str().unwrap();
//!         Ok(ToolOutput::text(format!("Processed: {value}")))
//!     }
//! }
//! ```
//!
//! # Module Structure
//!
//! - [`error`] - Error types for tool execution
//! - [`tool`] - Tool trait definition
//! - [`context`] - Execution context and approvals
//! - [`registry`] - Tool registry management
//! - [`executor`] - Streaming tool executor
//! - [`builtin`] - 16 built-in tools (Read, Glob, Grep, Edit, Write, Bash, Task, etc.)

pub mod builtin;
pub mod context;
pub mod error;
pub mod executor;
pub mod mcp_tool;
pub mod registry;
pub mod tool;

// Re-export main types at crate root
pub use context::{ApprovalStore, FileReadState, FileTracker, ToolContext, ToolContextBuilder};
pub use error::{Result, ToolError};
pub use executor::{ExecutorConfig, StreamingToolExecutor, ToolExecutionResult};
pub use mcp_tool::McpToolWrapper;
pub use registry::{McpToolInfo, ToolRegistry};
pub use tool::{Tool, ToolOutputExt};

// Re-export commonly used types from dependencies
pub use cocode_protocol::{
    AbortReason, ConcurrencySafety, ContextModifier, PermissionMode, PermissionResult, ToolOutput,
    ToolResultContent, ValidationResult,
};
pub use hyper_sdk::{ToolCall, ToolDefinition};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::builtin::{builtin_tool_names, register_builtin_tools};
    pub use crate::context::ToolContext;
    pub use crate::error::{Result, ToolError};
    pub use crate::executor::{ExecutorConfig, StreamingToolExecutor};
    pub use crate::registry::ToolRegistry;
    pub use crate::tool::{Tool, ToolOutputExt};
    pub use crate::{ConcurrencySafety, PermissionMode, ToolCall, ToolDefinition, ToolOutput};
}
