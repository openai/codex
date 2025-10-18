use thiserror::Error;

#[derive(Debug, Error)]
pub enum UnifiedExecError {
    #[error("Failed to create unified exec session: {pty_error}")]
    CreateSession {
        #[source]
        pty_error: anyhow::Error,
    },
    #[error("Unknown session id {session_id}")]
    UnknownSessionId { session_id: i32 },
    #[error("failed to write to stdin for session {session_id}")]
    WriteToStdin { session_id: i32 },
    #[error("missing command line for unified exec request")]
    MissingCommandLine,
    #[error("spawned process did not report a process id")]
    MissingProcessId,
    #[error("spawned process id {process_id} does not fit in i32")]
    ProcessIdOverflow { process_id: u32 },
    #[error("session {session_id} has already exited")]
    SessionExited {
        session_id: i32,
        exit_code: Option<i32>,
    },
}

impl UnifiedExecError {
    pub(crate) fn create_session(error: anyhow::Error) -> Self {
        Self::CreateSession { pty_error: error }
    }
}
