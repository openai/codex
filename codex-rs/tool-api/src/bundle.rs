use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;

use crate::FunctionToolSpec;
use crate::ToolCall;
use crate::ToolError;

/// Future returned by one contributed function-tool invocation.
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send + 'a>>;

/// Model-visible definition plus executable implementation for one contributed
/// function tool.
#[derive(Clone)]
pub struct ToolBundle {
    spec: FunctionToolSpec,
    executor: Arc<dyn ToolExecutor>,
}

impl ToolBundle {
    /// Creates one contributed function-tool bundle.
    pub fn new(spec: FunctionToolSpec, executor: Arc<dyn ToolExecutor>) -> Self {
        Self { spec, executor }
    }

    /// Returns the contributed function-tool spec.
    pub fn spec(&self) -> &FunctionToolSpec {
        &self.spec
    }

    /// Returns the contributed function-tool name.
    pub fn tool_name(&self) -> &str {
        self.spec.name.as_str()
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
    use crate::FunctionToolSpec;
    use crate::ToolCall;

    struct StubExecutor;

    impl ToolExecutor for StubExecutor {
        fn execute<'a>(&'a self, _call: ToolCall) -> ToolFuture<'a> {
            Box::pin(async { Ok(json!({ "ok": true })) })
        }
    }

    #[test]
    fn bundle_derives_name_from_function_spec() {
        let bundle = ToolBundle::new(
            FunctionToolSpec {
                name: "echo".to_string(),
                description: "Echo arguments.".to_string(),
                strict: false,
                parameters: json!({ "type": "object" }),
            },
            Arc::new(StubExecutor),
        );

        assert_eq!(bundle.tool_name(), "echo");
    }
}
