use codex_protocol::ThreadId;
use std::io::Error as IoError;
use std::io::ErrorKind;

/// Failure modes from loading a rollout for one expected thread.
#[derive(Debug)]
pub enum LoadRolloutItemsForThreadError {
    /// The rollout could not be read.
    Io(std::io::Error),
    /// The first session metadata record belongs to another thread.
    ThreadIdMismatch {
        /// Thread found in the rollout.
        actual_thread_id: ThreadId,
    },
    /// The rollout contained no session metadata record.
    MissingSessionMeta,
}

impl LoadRolloutItemsForThreadError {
    pub(crate) fn into_io_error(self) -> std::io::Error {
        match self {
            Self::Io(err) => err,
            err @ (Self::ThreadIdMismatch { .. } | Self::MissingSessionMeta) => {
                IoError::new(ErrorKind::InvalidData, err)
            }
        }
    }
}

impl std::fmt::Display for LoadRolloutItemsForThreadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => err.fmt(formatter),
            Self::ThreadIdMismatch { actual_thread_id } => {
                write!(
                    formatter,
                    "rollout contains history for thread {actual_thread_id}"
                )
            }
            Self::MissingSessionMeta => formatter.write_str("rollout contains no session metadata"),
        }
    }
}

impl std::error::Error for LoadRolloutItemsForThreadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::ThreadIdMismatch { .. } | Self::MissingSessionMeta => None,
        }
    }
}

impl From<std::io::Error> for LoadRolloutItemsForThreadError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
