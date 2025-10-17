use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum UnifiedExecError {
    #[error("Failed to create unified exec session: {message}")]
    CreateSession { message: String },
    #[error("Unknown session id {session_id}")]
    UnknownSessionId { session_id: i32 },
    #[error("failed to write to stdin")]
    WriteToStdin,
    #[error("missing command line for unified exec request")]
    MissingCommandLine,
    #[error("Command denied by sandbox: {message}")]
    SandboxDenied { message: String },
}

impl UnifiedExecError {
    pub(crate) fn create_session(message: String) -> Self {
        Self::CreateSession { message }
    }

    pub(crate) fn sandbox_denied(message: String) -> Self {
        Self::SandboxDenied { message }
    }
}
