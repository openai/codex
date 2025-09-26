use serde::Deserialize;
use serde::Serialize;
use std::fmt;

use super::session_id::SessionId;

#[derive(Debug, Clone, Deserialize)]
pub struct ExecControlParams {
    pub(crate) session_id: SessionId,
    pub(crate) action: ExecControlAction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecControlAction {
    Keepalive {
        #[serde(default)]
        extend_timeout_ms: Option<u64>,
    },
    SendCtrlC,
    Terminate,
    ForceKill,
    SetIdleTimeout {
        timeout_ms: u64,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecControlResponse {
    pub session_id: SessionId,
    pub status: ExecControlStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecControlStatus {
    Ack,
    NoSuchSession,
    AlreadyTerminated,
    Reject(String),
}

impl ExecControlStatus {
    pub(crate) fn ack() -> Self {
        Self::Ack
    }

    pub(crate) fn reject(msg: impl Into<String>) -> Self {
        Self::Reject(msg.into())
    }
}

impl fmt::Display for ExecControlStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ack => write!(f, "ack"),
            Self::NoSuchSession => write!(f, "no_such_session"),
            Self::AlreadyTerminated => write!(f, "already_terminated"),
            Self::Reject(msg) => write!(f, "reject({msg})"),
        }
    }
}
