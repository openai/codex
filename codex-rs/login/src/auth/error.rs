use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct RefreshTokenFailedError {
    pub reason: RefreshTokenFailedReason,
    pub message: String,
}

impl RefreshTokenFailedError {
    pub fn new(reason: RefreshTokenFailedReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshTokenFailedReason {
    Expired,
    Exhausted,
    Revoked,
    Other,
}
