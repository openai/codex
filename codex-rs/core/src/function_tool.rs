use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum FunctionCallError {
    #[error("{0}")]
    RespondToModel(String),
    #[error("Fatal error: {0}")]
    Fatal(String),
}
