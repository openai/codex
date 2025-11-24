use crate::ExportError;
use std::sync::PoisonError;
use std::time;
use thiserror::Error;

/// A specialized `Result` type for trace operations.
pub type TraceResult<T> = Result<T, TraceError>;

/// Errors returned by the trace API.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TraceError {
    /// Export failed with the error returned by the exporter
    #[error("Exporter {0} encountered the following error(s): {name}", name = .0.exporter_name())]
    ExportFailed(Box<dyn ExportError>),

    /// Export failed to finish after certain period and processor stopped the export.
    #[error("Exporting timed out after {} seconds", .0.as_secs())]
    ExportTimedOut(time::Duration),

    /// already shutdown error
    #[error("TracerProvider already shutdown")]
    TracerProviderAlreadyShutdown,

    /// Other errors propagated from trace SDK that weren't covered above
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl<T> From<T> for TraceError
where
    T: ExportError,
{
    fn from(err: T) -> Self {
        TraceError::ExportFailed(Box::new(err))
    }
}

impl From<String> for TraceError {
    fn from(err_msg: String) -> Self {
        TraceError::Other(err_msg.into())
    }
}

impl From<&'static str> for TraceError {
    fn from(err_msg: &'static str) -> Self {
        TraceError::Other(Box::new(Custom(err_msg.into())))
    }
}

impl<T> From<PoisonError<T>> for TraceError {
    fn from(err: PoisonError<T>) -> Self {
        TraceError::Other(err.to_string().into())
    }
}

/// Wrap type for string
#[derive(Error, Debug)]
#[error("{0}")]
struct Custom(String);
