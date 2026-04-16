use codex_protocol::ThreadId;

/// Result type returned by thread-store operations.
pub type ThreadStoreResult<T> = Result<T, ThreadStoreError>;

/// Error type shared by thread-store implementations.
#[derive(Debug, thiserror::Error)]
pub enum ThreadStoreError {
    /// The requested thread does not exist in this store.
    #[error("thread {thread_id} not found")]
    ThreadNotFound {
        /// Thread id requested by the caller.
        thread_id: ThreadId,
    },

    /// The caller supplied invalid request data.
    #[error("invalid thread-store request: {message}")]
    InvalidRequest {
        /// User-facing explanation of the invalid request.
        message: String,
    },

    /// The operation conflicted with current store state.
    #[error("thread-store conflict: {message}")]
    Conflict {
        /// User-facing explanation of the conflict.
        message: String,
    },

    /// Catch-all for implementation failures that do not fit a more specific category.
    #[error("thread-store internal error: {message}")]
    Internal {
        /// User-facing explanation of the implementation failure.
        message: String,
    },
}
