use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum FunctionCallError {
    #[error("{0}")]
// core/src/function_tool.rs
    RespondToModel(String),
    #[error("LocalShellCall without call_id or id")]
    MissingLocalShellCallId,
    #[error("Fatal error: {0}")]
    Fatal(String),
}
