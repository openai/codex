use codex_protocol::ToolName;
use serde_json::Value;

use crate::FunctionToolSpec;

/// One callable function tool, its exposure mode, and the runtime object that
/// executes it.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolDefinition<R> {
    tool_name: ToolName,
    spec: FunctionToolSpec,
    output_schema: Option<Value>,
    exposure: ToolExposure,
    runtime: R,
}

/// Whether a tool is advertised immediately or marked for deferred loading.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ToolExposure {
    #[default]
    Published,
    Deferred,
}

impl<R> ToolDefinition<R> {
    /// Creates one immediately-published function tool definition.
    pub fn new(tool_name: ToolName, spec: FunctionToolSpec, runtime: R) -> Self {
        debug_assert_eq!(
            tool_name.name, spec.name,
            "tool definition name must match its function spec name"
        );
        Self {
            tool_name,
            spec,
            output_schema: None,
            exposure: ToolExposure::Published,
            runtime,
        }
    }

    /// Returns the callable tool name, including any namespace.
    pub fn tool_name(&self) -> &ToolName {
        &self.tool_name
    }

    /// Returns the function-tool metadata exposed to the model.
    pub fn spec(&self) -> &FunctionToolSpec {
        &self.spec
    }

    /// Returns the optional tool-output schema kept alongside the model spec.
    pub fn output_schema(&self) -> Option<&Value> {
        self.output_schema.as_ref()
    }

    /// Returns how this tool should be exposed to the model.
    pub fn exposure(&self) -> ToolExposure {
        self.exposure
    }

    /// Returns the runtime object bound to this tool definition.
    pub fn runtime(&self) -> &R {
        &self.runtime
    }

    /// Rebinds the same tool definition to a different runtime object.
    pub fn with_runtime<S>(self, runtime: S) -> ToolDefinition<S> {
        ToolDefinition {
            tool_name: self.tool_name,
            spec: self.spec,
            output_schema: self.output_schema,
            exposure: self.exposure,
            runtime,
        }
    }

    /// Attaches a tool-output schema.
    pub fn with_output_schema(mut self, output_schema: Value) -> Self {
        self.output_schema = Some(output_schema);
        self
    }

    /// Marks this tool as deferred. Deferred tools intentionally omit output
    /// schema metadata until they are loaded.
    pub fn deferred(mut self) -> Self {
        self.exposure = ToolExposure::Deferred;
        self.output_schema = None;
        self
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::ToolDefinition;
    use super::ToolExposure;
    use crate::FunctionToolSpec;
    use crate::ToolName;

    fn definition() -> ToolDefinition<()> {
        ToolDefinition::new(
            ToolName::namespaced("mcp__calendar", "create_event"),
            FunctionToolSpec {
                name: "create_event".to_string(),
                description: "Create an event.".to_string(),
                strict: false,
                parameters: json!({ "type": "object" }),
            },
            (),
        )
        .with_output_schema(json!({ "type": "object" }))
    }

    #[test]
    fn deferred_tools_drop_output_schema() {
        let definition = definition().deferred();

        assert_eq!(definition.exposure(), ToolExposure::Deferred);
        assert_eq!(definition.output_schema(), None);
    }

    #[test]
    fn runtime_can_be_rebound_without_rebuilding_metadata() {
        let definition = definition().with_runtime("handler");

        assert_eq!(
            definition.tool_name(),
            &ToolName::namespaced("mcp__calendar", "create_event")
        );
        assert_eq!(definition.runtime(), &"handler");
    }
}
