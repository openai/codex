use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadHistoryMode;

/// Result type returned by thread-store operations.
pub type ThreadStoreResult<T> = Result<T, ThreadStoreError>;

pub(crate) fn reject_paginated_history_mode(
    history_mode: ThreadHistoryMode,
) -> ThreadStoreResult<()> {
    if matches!(history_mode, ThreadHistoryMode::Paginated) {
        return Err(ThreadStoreError::Unsupported {
            operation: "paginated_threads",
        });
    }
    Ok(())
}

/// Error type shared by thread-store implementations.
///
/// Implementations must explicitly distinguish known transient failures from writes whose
/// outcome is unknown. These categories carry information only: the operation owner decides
/// whether and how to recover, and no variant automatically triggers a retry.
#[derive(Clone, Debug, thiserror::Error)]
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

    /// The operation conflicted with current store state. This alone does not imply retryability.
    #[error("thread-store conflict: {message}")]
    Conflict {
        /// User-facing explanation of the conflict.
        message: String,
    },

    /// The store implementation does not support this operation yet.
    #[error("thread-store unsupported operation: {operation}")]
    Unsupported {
        /// Stable operation name for callers that need to map unsupported operations.
        operation: &'static str,
    },

    /// A transient failure for which retrying the operation is safe.
    ///
    /// For a write, the implementation must know the failed attempt did not commit. Otherwise
    /// use [`Self::OutcomeUnknown`]. Retry scheduling and limits belong to the operation owner.
    #[error("thread-store retryable error: {message}")]
    Retryable {
        /// User-facing explanation of the transient failure.
        message: String,
    },

    /// A write failed to report a definitive outcome and may already have committed.
    ///
    /// The operation owner must reconcile the stored state or use an idempotent replay strategy
    /// before retrying. This must not be treated as an ordinary retryable failure.
    #[error("thread-store write outcome unknown: {message}")]
    OutcomeUnknown {
        /// User-facing explanation of why the write outcome is uncertain.
        message: String,
    },

    /// Catch-all for implementation failures that do not fit a more specific category.
    ///
    /// This includes unclassified failures: callers must not infer retryability or whether a
    /// write committed from this variant or its message.
    #[error("thread-store internal error: {message}")]
    Internal {
        /// User-facing explanation of the implementation failure.
        message: String,
    },
}

impl ThreadStoreError {
    /// Whether a read found no persisted record for this specific thread.
    pub fn is_missing_for_read(&self, requested_thread_id: ThreadId) -> bool {
        match self {
            Self::ThreadNotFound { thread_id } => *thread_id == requested_thread_id,
            // The local store's strict read uses this legacy error for an unmaterialized rollout.
            Self::InvalidRequest { message } => {
                message == &format!("no rollout found for thread id {requested_thread_id}")
            }
            Self::Conflict { .. }
            | Self::Unsupported { .. }
            | Self::Retryable { .. }
            | Self::OutcomeUnknown { .. }
            | Self::Internal { .. } => false,
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
