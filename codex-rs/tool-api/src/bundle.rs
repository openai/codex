use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use codex_protocol::ToolName;

use crate::ExecutableToolSpec;
use crate::ToolCall;
use crate::ToolError;
use crate::ToolOutput;

/// Future returned by one executable-tool invocation.
pub type ToolFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Box<dyn ToolOutput>, ToolError>> + Send + 'a>>;

/// Future returned by one mutability probe.
pub type BoolFuture<'a> = Pin<Box<dyn Future<Output = bool> + Send + 'a>>;

/// Model-visible definition plus executable implementation for one tool.
#[derive(Clone)]
pub struct ToolBundle {
    spec: ExecutableToolSpec,
    supports_parallel_tool_calls: bool,
    executor: Arc<dyn ToolExecutor>,
}

impl ToolBundle {
    /// Creates one executable tool bundle.
    pub fn new(spec: impl Into<ExecutableToolSpec>, executor: Arc<dyn ToolExecutor>) -> Self {
        Self {
            spec: spec.into(),
            supports_parallel_tool_calls: false,
            executor,
        }
    }

    /// Marks this tool as safe for the host to run in parallel with peers.
    #[must_use]
    pub fn allow_parallel_calls(mut self) -> Self {
        self.supports_parallel_tool_calls = true;
        self
    }

    /// Returns the executable tool spec.
    pub fn spec(&self) -> &ExecutableToolSpec {
        &self.spec
    }

    /// Returns the callable tool name derived from the spec.
    pub fn tool_name(&self) -> ToolName {
        self.spec.tool_name()
    }

    /// Returns whether the tool may run in parallel with peers.
    pub fn supports_parallel_tool_calls(&self) -> bool {
        self.supports_parallel_tool_calls
    }

    /// Returns the executable implementation.
    pub fn executor(&self) -> Arc<dyn ToolExecutor> {
        Arc::clone(&self.executor)
    }
}

/// Executable behavior for one contributed tool.
///
/// Implementations receive the model-supplied call id and input and return a
/// host-renderable tool result.
pub trait ToolExecutor: Send + Sync {
    fn execute<'a>(&'a self, call: ToolCall) -> ToolFuture<'a>;

    /// Returns whether the call may mutate user state.
    ///
    /// Hosts can use this conservative signal for serialization or approval
    /// policy. Context-free read tools should keep the default.
    fn is_mutating<'a>(&'a self, _call: &'a ToolCall) -> BoolFuture<'a> {
        Box::pin(async { false })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex_protocol::ToolName;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::ToolBundle;
    use super::ToolExecutor;
    use super::ToolFuture;
    use crate::FreeformToolFormat;
    use crate::FreeformToolSpec;
    use crate::FunctionToolSpec;
    use crate::JsonSchema;
    use crate::JsonToolOutput;
    use crate::ToolCall;

    struct StubExecutor;

    impl ToolExecutor for StubExecutor {
        fn execute<'a>(&'a self, _call: ToolCall) -> ToolFuture<'a> {
            Box::pin(async {
                Ok::<Box<dyn crate::ToolOutput>, crate::ToolError>(Box::new(JsonToolOutput::new(
                    json!({ "ok": true }),
                )))
            })
        }
    }

    #[test]
    fn bundle_derives_name_from_function_spec() {
        let bundle = ToolBundle::new(
            FunctionToolSpec {
                name: "echo".to_string(),
                description: "Echo arguments.".to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(
                    Default::default(),
                    /*required*/ None,
                    /*additional_properties*/ None,
                ),
                output_schema: None,
            },
            Arc::new(StubExecutor),
        );

        assert_eq!(bundle.tool_name(), ToolName::plain("echo"));
    }

    #[test]
    fn bundle_derives_name_from_freeform_spec() {
        let bundle = ToolBundle::new(
            FreeformToolSpec {
                name: "apply_patch".to_string(),
                description: "Apply a patch.".to_string(),
                format: FreeformToolFormat {
                    r#type: "grammar".to_string(),
                    syntax: "lark".to_string(),
                    definition: "start: patch".to_string(),
                },
            },
            Arc::new(StubExecutor),
        );

        assert_eq!(bundle.tool_name(), ToolName::plain("apply_patch"));
    }
}
