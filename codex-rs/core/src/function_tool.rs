use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum FunctionCallError {
    #[error("turn interrupted")]
    Interrupted,
    #[error("{0}")]
    RespondToModel(String),
    #[error("LocalShellCall without call_id or id")]
    MissingLocalShellCallId,
    #[error("Fatal error: {0}")]
    Fatal(String),
}
