use thiserror::Error;

pub type Result<T> = std::result::Result<T, MetricsError>;

#[derive(Debug, Error)]
pub enum MetricsError {
    // Buckets.
    #[error("histogram buckets cannot be empty")]
    EmptyBuckets,
    #[error("histogram bucket step must be positive: {step}")]
    BucketStepNonPositive { step: i64 },
    #[error("histogram bucket start must be positive: {start}")]
    BucketStartNonPositive { start: i64 },
    #[error("histogram bucket factor must be finite and greater than 1: {factor}")]
    BucketFactorInvalid { factor: f64 },
    #[error("histogram bucket range must be ascending: {from}..={to}")]
    BucketRangeDescending { from: i64, to: i64 },
    #[error("histogram bucket range overflow: {from}..={to} step {step}")]
    BucketRangeOverflow { from: i64, to: i64, step: i64 },

    // Metrics.
    #[error("metric name cannot be empty")]
    EmptyMetricName,
    #[error("metric name contains invalid characters: {name}")]
    InvalidMetricName { name: String },
    #[error("{label} cannot be empty")]
    EmptyTagComponent { label: String },
    #[error("{label} contains invalid characters: {value}")]
    InvalidTagComponent { label: String, value: String },
    #[error("tag key is reserved: {key}")]
    ReservedTagKey { key: String },

    // Config.
    #[error("metrics endpoint cannot be empty")]
    EmptyEndpoint,
    #[error("metrics api key cannot be empty")]
    EmptyApiKey,
    #[error("failed to build metrics exporter")]
    ExporterBuild {
        #[source]
        source: opentelemetry_otlp::ExporterBuildError,
    },
    #[error("failed to flush metrics")]
    FlushFailed {
        #[source]
        source: opentelemetry_sdk::error::OTelSdkError,
    },
    #[error("failed to shutdown metrics provider")]
    ShutdownFailed {
        #[source]
        source: opentelemetry_sdk::error::OTelSdkError,
    },

    // Worker.
    #[error("metrics queue capacity must be positive")]
    QueueCapacityZero,
    #[error("metrics queue is full (capacity {capacity})")]
    QueueFull { capacity: usize },
    #[error("metrics worker is unavailable")]
    WorkerUnavailable,
    #[error("metrics worker thread panicked")]
    WorkerPanicked,
}
