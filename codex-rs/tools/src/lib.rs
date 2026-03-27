//! Shared tool-schema parsing primitives that can live outside `codex-core`.

mod json_schema;

pub use json_schema::AdditionalProperties;
pub use json_schema::JsonSchema;
pub use json_schema::parse_tool_input_schema;
