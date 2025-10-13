use codex_protocol::ConversationId;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SubsessionError {
    #[error("unknown session {session_id}")]
    UnknownSession { session_id: String },
    #[error("session {session_id} is still running")]
    Pending { session_id: String },
    #[error("session {session_id} timed out after {timeout_ms}ms")]
    Timeout { session_id: String, timeout_ms: u64 },
    #[error("failed to spawn child session: {message}")]
    SpawnFailed { message: String },
    #[error("child session {session_id} cancelled")]
    Cancelled { session_id: String },
    #[error("missing auth manager for child sessions")]
    MissingAuthManager,
}

impl SubsessionError {
    pub(crate) fn unknown(id: &ConversationId) -> Self {
        Self::UnknownSession {
            session_id: id.to_string(),
        }
    }

    pub(crate) fn pending(id: &ConversationId) -> Self {
        Self::Pending {
            session_id: id.to_string(),
        }
    }

    pub(crate) fn cancelled(id: &ConversationId) -> Self {
        Self::Cancelled {
            session_id: id.to_string(),
        }
    }

    pub(crate) fn timeout(id: &ConversationId, timeout_ms: u64) -> Self {
        Self::Timeout {
            session_id: id.to_string(),
            timeout_ms,
        }
    }
}
