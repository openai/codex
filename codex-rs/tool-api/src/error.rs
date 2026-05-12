use thiserror::Error;

/// Error returned by a contributed executable tool.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ToolError {
    #[error("{0}")]
    RespondToModel(String),
    #[error("fatal tool error: {0}")]
    Fatal(String),
}

impl ToolError {
    /// Creates a model-visible tool error.
    pub fn respond_to_model(message: impl Into<String>) -> Self {
        Self::RespondToModel(message.into())
    }

    /// Creates a host-fatal tool error.
    pub fn fatal(message: impl Into<String>) -> Self {
        Self::Fatal(message.into())
    }
}
