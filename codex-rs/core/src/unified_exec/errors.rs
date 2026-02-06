use crate::exec::ExecToolCallOutput;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum UnifiedExecError {
    #[error("Failed to create unified exec session: {message}")]
    CreateSession { message: String },
    // Called "session" in the model's training.
    #[error("Unknown session id {process_id}")]
    UnknownSessionId { process_id: String },
    #[error("failed to write to stdin")]
    WriteToStdin,
    #[error("missing command line for unified exec request")]
    MissingCommandLine,
    #[error("Command denied by sandbox: {message}")]
    SandboxDenied {
        message: String,
        output: ExecToolCallOutput,
    },
}

impl UnifiedExecError {
    pub(crate) fn create_session(message: String) -> Self {
        Self::CreateSession { message }
    }

    pub(crate) fn sandbox_denied(message: String, output: ExecToolCallOutput) -> Self {
        Self::SandboxDenied { message, output }
    }
}
