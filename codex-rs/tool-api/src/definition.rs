use codex_protocol::ToolName;
use serde_json::Value;

use crate::FunctionToolSpec;

/// One callable function tool, its exposure mode, and the runtime object that
/// executes it.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolDefinition<R, S = FunctionToolSpec> {
    tool_name: ToolName,
    spec: S,
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

impl<R, S> ToolDefinition<R, S> {
    /// Creates one immediately-published tool definition.
    pub fn new(tool_name: ToolName, spec: S, runtime: R) -> Self {
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

    /// Returns the model-visible metadata bound to this definition.
    pub fn spec(&self) -> &S {
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
    pub fn with_runtime<T>(self, runtime: T) -> ToolDefinition<T, S> {
        ToolDefinition {
            tool_name: self.tool_name,
            spec: self.spec,
            output_schema: self.output_schema,
            exposure: self.exposure,
            runtime,
        }
    }

    /// Marks this tool as deferred. Deferred tools intentionally omit output
    /// schema metadata until they are loaded.
    pub fn deferred(mut self) -> Self {
        self.exposure = ToolExposure::Deferred;
        self.output_schema = None;
        self
    }
}

impl<R> ToolDefinition<R> {
    /// Creates one immediately-published flat function-tool definition and
    /// derives its callable name from the function spec.
    pub fn from_function_spec(spec: FunctionToolSpec, runtime: R) -> Self {
        let tool_name = ToolName::plain(spec.name.clone());
        Self::new(tool_name, spec, runtime)
    }

    /// Attaches a tool-output schema to an ordinary function definition.
    pub fn with_output_schema(mut self, output_schema: Value) -> Self {
        self.output_schema = Some(output_schema);
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

    #[test]
    fn flat_function_definitions_derive_their_tool_name_from_the_spec() {
        let definition = ToolDefinition::from_function_spec(
            FunctionToolSpec {
                name: "echo".to_string(),
                description: "Echo arguments.".to_string(),
                strict: false,
                parameters: json!({ "type": "object" }),
            },
            (),
        );

        assert_eq!(definition.tool_name(), &ToolName::plain("echo"));
    }
}
