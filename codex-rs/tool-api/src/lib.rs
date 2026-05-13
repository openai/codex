//! Minimal function-tool contracts shared between hosts and extension-owned
//! tool crates.

mod bundle;
mod call;
mod error;
mod spec;

pub use bundle::ToolBundle;
pub use bundle::ToolExecutor;
pub use bundle::ToolFuture;
pub use call::ToolCall;
pub use error::ToolError;
pub use spec::FunctionToolSpec;
