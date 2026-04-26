/// Result type returned by agent graph store operations.
pub type AgentGraphStoreResult<T> = Result<T, AgentGraphStoreError>;

/// Error type shared by agent graph store implementations.
#[derive(Debug, thiserror::Error)]
pub enum AgentGraphStoreError {
    /// The caller supplied invalid request data.
    #[error("invalid agent graph store request: {message}")]
    InvalidRequest {
        /// User-facing explanation of the invalid request.
        message: String,
    },

    /// Catch-all for implementation failures that do not fit a more specific category.
    #[error("agent graph store internal error: {message}")]
    Internal {
        /// User-facing explanation of the implementation failure.
        message: String,
    },
}
