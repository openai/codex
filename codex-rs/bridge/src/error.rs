/// Result type returned by bridge operations.
pub type BridgeResult<T> = Result<T, BridgeError>;

/// Errors surfaced by the local Rust-to-Python bridge.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    /// MsgPack serialization or deserialization failed.
    #[error("bridge messagepack error: {message}")]
    Codec {
        /// Codec error detail.
        message: String,
    },

    /// The transport failed while reading or writing frames.
    #[error("bridge transport error: {message}")]
    Transport {
        /// Transport error detail.
        message: String,
    },

    /// The remote service returned an application error.
    #[error("remote bridge method `{method}` failed: {message}")]
    Remote {
        /// Method that failed.
        method: String,
        /// Remote error code.
        code: String,
        /// Remote error detail.
        message: String,
    },

    /// The remote response did not match the requested operation.
    #[error("invalid bridge response: {message}")]
    InvalidResponse {
        /// Validation error detail.
        message: String,
    },
}
