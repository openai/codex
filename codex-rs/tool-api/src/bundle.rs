use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;

use crate::JsonSchema;
use crate::ToolCall;
use crate::ToolError;
use crate::ToolName;
use crate::ToolNamespace;

/// Future returned by one contributed function-tool invocation.
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send + 'a>>;

/// Model-visible definition plus executable implementation for one contributed
/// function tool.
#[derive(Clone)]
pub struct ToolBundle {
    tool_name: ToolName,
    namespace: Option<ToolNamespace>,
    description: String,
    input_schema: JsonSchema,
    strict: bool,
    output_schema: Option<Value>,
    defer_loading: bool,
    executor: Arc<dyn ToolExecutor>,
}

impl ToolBundle {
    /// Creates one contributed function-tool bundle.
    pub fn new(
        tool_name: ToolName,
        description: String,
        input_schema: JsonSchema,
        executor: Arc<dyn ToolExecutor>,
    ) -> Self {
        let namespace = tool_name
            .namespace
            .as_ref()
            .map(|namespace| ToolNamespace::new(namespace.clone()));
        Self {
            tool_name,
            namespace,
            description,
            input_schema,
            strict: false,
            output_schema: None,
            defer_loading: false,
            executor,
        }
    }

    /// Enables strict schema handling for this contributed tool.
    pub fn strict(mut self) -> Self {
        self.strict = true;
        self
    }

    /// Adds a model-visible output schema for this contributed tool.
    pub fn with_output_schema(mut self, output_schema: Value) -> Self {
        self.output_schema = Some(output_schema);
        self
    }

    /// Marks this contributed tool as loadable on demand.
    pub fn deferred(mut self) -> Self {
        self.defer_loading = true;
        self
    }

    /// Places this contributed tool in a model-visible namespace.
    pub fn in_namespace(mut self, namespace: ToolNamespace) -> Self {
        self.tool_name.namespace = Some(namespace.name().to_string());
        self.namespace = Some(namespace);
        self
    }

    /// Returns the contributed function-tool name.
    pub fn tool_name(&self) -> &ToolName {
        &self.tool_name
    }

    /// Returns the contributed tool namespace, if any.
    pub fn namespace(&self) -> Option<&ToolNamespace> {
        self.namespace.as_ref()
    }

    /// Returns the contributed function-tool description.
    pub fn description(&self) -> &str {
        self.description.as_str()
    }

    /// Returns the contributed function-tool input schema.
    pub fn input_schema(&self) -> &JsonSchema {
        &self.input_schema
    }

    /// Returns whether strict schema handling is enabled.
    pub fn is_strict(&self) -> bool {
        self.strict
    }

    /// Returns the optional contributed function-tool output schema.
    pub fn output_schema(&self) -> Option<&Value> {
        self.output_schema.as_ref()
    }

    /// Returns whether the contributed function tool should be loadable on demand.
    pub fn defer_loading(&self) -> bool {
        self.defer_loading
    }

    /// Returns the executable implementation.
    pub fn executor(&self) -> Arc<dyn ToolExecutor> {
        Arc::clone(&self.executor)
    }
}

/// Executable behavior for one contributed function tool.
///
/// Implementations receive the model-supplied call id and JSON arguments and
/// return the JSON value that should be exposed to the model.
pub trait ToolExecutor: Send + Sync {
    fn execute<'a>(&'a self, call: ToolCall) -> ToolFuture<'a>;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::ToolBundle;
    use super::ToolExecutor;
    use super::ToolFuture;
    use crate::JsonSchema;
    use crate::ToolCall;
    use crate::ToolName;
    use crate::ToolNamespace;

    struct StubExecutor;

    impl ToolExecutor for StubExecutor {
        fn execute<'a>(&'a self, _call: ToolCall) -> ToolFuture<'a> {
            Box::pin(async { Ok(json!({ "ok": true })) })
        }
    }

    #[test]
    fn bundle_preserves_plain_tool_name() {
        let bundle = ToolBundle::new(
            ToolName::plain("echo"),
            "Echo arguments.".to_string(),
            JsonSchema::object(
                std::collections::BTreeMap::new(),
                /*required*/ None,
                Some(true.into()),
            ),
            Arc::new(StubExecutor),
        );

        assert_eq!(bundle.tool_name(), &ToolName::plain("echo"));
        assert_eq!(bundle.namespace(), None);
    }

    #[test]
    fn bundle_derives_namespace_from_namespaced_tool_name() {
        let bundle = ToolBundle::new(
            ToolName::namespaced("extension_tools/", "echo"),
            "Echo arguments.".to_string(),
            JsonSchema::object(
                std::collections::BTreeMap::new(),
                /*required*/ None,
                Some(true.into()),
            ),
            Arc::new(StubExecutor),
        );

        assert_eq!(
            bundle.tool_name(),
            &ToolName::namespaced("extension_tools/", "echo")
        );
        assert_eq!(
            bundle.namespace(),
            Some(&ToolNamespace::new("extension_tools/"))
        );
    }

    #[test]
    fn bundle_sets_tool_name_when_namespace_metadata_is_attached() {
        let bundle = ToolBundle::new(
            ToolName::plain("echo"),
            "Echo arguments.".to_string(),
            JsonSchema::object(
                std::collections::BTreeMap::new(),
                /*required*/ None,
                Some(true.into()),
            ),
            Arc::new(StubExecutor),
        )
        .in_namespace(
            ToolNamespace::new("extension_tools/")
                .with_description("Extension-owned function tools."),
        );

        assert_eq!(
            bundle.tool_name(),
            &ToolName::namespaced("extension_tools/", "echo")
        );
        assert_eq!(
            bundle.namespace(),
            Some(
                &ToolNamespace::new("extension_tools/")
                    .with_description("Extension-owned function tools.")
            )
        );
    }
}
