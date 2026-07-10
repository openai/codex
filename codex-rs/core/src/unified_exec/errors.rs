use codex_protocol::exec_output::ExecToolCallOutput;
use codex_utils_path_uri::PathUri;
use std::num::NonZeroUsize;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum UnifiedExecError {
    #[error("Failed to create unified exec process: {message}")]
    CreateProcess { message: String },
    #[error("Unified exec process failed: {message}")]
    ProcessFailed { message: String },
    // The model is trained on `session_id`, but internally we track a `process_id`.
    #[error("Unknown process id {process_id}")]
    UnknownProcessId { process_id: i32 },
    #[error("failed to write to stdin")]
    WriteToStdin,
    #[error(
        "stdin is closed for this session; rerun exec_command with tty=true to keep stdin open"
    )]
    StdinClosed,
    #[error("missing command line for unified exec request")]
    MissingCommandLine,
    #[error("Command denied by sandbox: {message}")]
    SandboxDenied {
        message: String,
        output: ExecToolCallOutput,
        original_token_count: Option<usize>,
        output_omitted_bytes: Option<NonZeroUsize>,
    },
    #[error("{path} is not valid on {}", std::env::consts::OS)]
    ForeignPath { path: PathUri },
}

impl UnifiedExecError {
    pub(crate) fn create_process(message: String) -> Self {
        Self::CreateProcess { message }
    }

    pub(crate) fn process_failed(message: String) -> Self {
        Self::ProcessFailed { message }
    }

    pub(crate) fn sandbox_denied(message: String, output: ExecToolCallOutput) -> Self {
        Self::SandboxDenied {
            message,
            output,
            original_token_count: None,
            output_omitted_bytes: None,
        }
    }

    pub(crate) fn with_output_collection_metadata(
        self,
        original_token_count: usize,
        output_omitted_bytes: Option<NonZeroUsize>,
    ) -> Self {
        match self {
            Self::SandboxDenied {
                message, output, ..
            } => Self::SandboxDenied {
                message,
                output,
                original_token_count: Some(original_token_count),
                output_omitted_bytes,
            },
            other => other,
        }
    }
}
