use std::result;
use std::sync::PoisonError;
use thiserror::Error;

/// A specialized `Result` type for metric operations.
pub(crate) type MetricResult<T> = result::Result<T, MetricError>;

/// Errors returned by the metrics API.
#[derive(Error, Debug)]
pub(crate) enum MetricError {
    /// Other errors not covered by specific cases.
    #[error("Metrics error: {0}")]
    Other(String),
    /// Invalid configuration
    #[error("Config error {0}")]
    Config(String),
    /// Invalid instrument configuration such invalid instrument name, invalid instrument description, invalid instrument unit, etc.
    /// See [spec](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/api.md#general-characteristics)
    /// for full list of requirements.
    #[error("Invalid instrument configuration: {0}")]
    InvalidInstrumentConfiguration(&'static str),
}

impl<T> From<PoisonError<T>> for MetricError {
    fn from(err: PoisonError<T>) -> Self {
        MetricError::Other(err.to_string())
    }
}
