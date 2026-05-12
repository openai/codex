use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::ToolCall;
use crate::ToolError;

/// Future returned by one contributed function-tool invocation.
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send + 'a>>;

/// Executable behavior for one contributed function tool.
///
/// Implementations receive the model-supplied call id and JSON arguments and
/// return the JSON value that should be exposed to the model.
pub trait ToolExecutor: Send + Sync {
    fn execute<'a>(&'a self, call: ToolCall) -> ToolFuture<'a>;
}
