/// Result type returned by config-store operations.
pub type ConfigStoreResult<T> = Result<T, ConfigStoreError>;

/// Error type shared by config-store implementations.
#[derive(Debug, thiserror::Error)]
pub enum ConfigStoreError {
    /// The caller supplied invalid request data.
    #[error("invalid config-store request: {message}")]
    InvalidRequest {
        /// User-facing explanation of the invalid request.
        message: String,
    },

    /// A backing source could not be queried.
    #[error("config-store read failed: {message}")]
    ReadFailed {
        /// User-facing explanation of the read failure.
        message: String,
    },

    /// Catch-all for implementation failures that do not fit a more specific category.
    #[error("config-store internal error: {message}")]
    Internal {
        /// User-facing explanation of the implementation failure.
        message: String,
    },
}
