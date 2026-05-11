//! Minimal function-tool contracts shared between hosts and extension-owned
//! tool crates.

mod bundle;
mod call;
mod error;
mod json_schema;
mod namespace;

pub use bundle::ToolBundle;
pub use bundle::ToolExecutor;
pub use bundle::ToolFuture;
pub use call::ToolCall;
pub use codex_protocol::ToolName;
pub use error::ToolError;
pub use json_schema::AdditionalProperties;
pub use json_schema::JsonSchema;
pub use json_schema::JsonSchemaPrimitiveType;
pub use json_schema::JsonSchemaType;
pub use json_schema::parse_tool_input_schema;
pub use namespace::ToolNamespace;
