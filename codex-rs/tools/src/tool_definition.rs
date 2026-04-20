use crate::JsonSchema;
use crate::ToolName;
use serde_json::Value as JsonValue;

/// Canonical metadata for JSON-schema function tools.
///
/// This intentionally models function-like tools only. If freeform tools need
/// the same registry/search/code-mode lifecycle later, this can grow a
/// function-vs-freeform input enum without changing the conversion boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDefinition {
    pub name: ToolName,
    pub description: String,
    pub input_schema: JsonSchema,
    pub output_schema: Option<JsonValue>,
    pub loading: ToolLoadingPolicy,
    pub execution: ToolExecution,
    pub presentation: Option<ToolPresentation>,
    pub search: Option<ToolSearchMetadata>,
    pub supports_parallel_tool_calls: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolLoadingPolicy {
    Eager,
    Deferred,
}

impl ToolLoadingPolicy {
    pub fn is_deferred(self) -> bool {
        matches!(self, Self::Deferred)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecution {
    /// Tool execution handled by an in-process Codex handler.
    Builtin,
    /// Tool registered dynamically by the caller for the current thread.
    Dynamic,
    /// Tool routed through the MCP connection manager.
    Mcp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPresentation {
    pub namespace_display_name: Option<String>,
    pub namespace_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSearchMetadata {
    pub source_name: String,
    pub source_description: Option<String>,
    pub extra_terms: Vec<String>,
    pub limit_bucket: Option<String>,
}

impl ToolDefinition {
    pub fn renamed(mut self, name: impl Into<ToolName>) -> Self {
        self.name = name.into();
        self
    }

    pub fn into_deferred(mut self) -> Self {
        self.output_schema = None;
        self.loading = ToolLoadingPolicy::Deferred;
        self
    }

    pub fn defer_loading(&self) -> bool {
        self.loading.is_deferred()
    }
}

#[cfg(test)]
#[path = "tool_definition_tests.rs"]
mod tests;
