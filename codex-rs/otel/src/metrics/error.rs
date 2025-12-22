use std::time::Duration;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, MetricsError>;

#[derive(Debug, Error)]
pub enum MetricsError {
    // Metrics.
    #[error("metric name cannot be empty")]
    EmptyMetricName,
    #[error("metric name contains invalid characters: {name}")]
    InvalidMetricName { name: String },
    #[error("{label} cannot be empty")]
    EmptyTagComponent { label: String },
    #[error("{label} contains invalid characters: {value}")]
    InvalidTagComponent { label: String, value: String },

    // Config.
    #[error("failed to build tokio runtime")]
    RuntimeBuild {
        #[source]
        source: std::io::Error,
    },
    #[error("invalid api key header: {header}")]
    InvalidApiKeyHeader {
        header: String,
        #[source]
        source: reqwest::header::InvalidHeaderName,
    },
    #[error("invalid header value: {header}")]
    InvalidHeaderValue {
        header: String,
        #[source]
        source: reqwest::header::InvalidHeaderValue,
    },
    #[error("failed to build metrics http client")]
    HttpClientBuild {
        #[source]
        source: reqwest::Error,
    },
    #[error("metrics endpoint cannot be empty")]
    EmptyEndpoint,
    #[error("metrics api key cannot be empty")]
    EmptyApiKey,

    // Worker.
    #[error("metrics queue capacity must be positive")]
    QueueCapacityZero,
    #[error("metrics queue is full (capacity {capacity})")]
    QueueFull { capacity: usize },
    #[error("metrics worker is unavailable")]
    WorkerUnavailable,
    #[error("metrics worker thread panicked")]
    WorkerPanicked,
    #[error("metrics shutdown timed out after {timeout:?}")]
    ShutdownTimeout { timeout: Duration },
    #[error("failed to send statsig metrics request")]
    StatsigRequestFailed {
        #[source]
        source: reqwest::Error,
    },
    #[error("statsig metrics request failed: {status} {body}")]
    StatsigResponseError {
        status: reqwest::StatusCode,
        body: String,
    },
}
