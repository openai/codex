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

    #[error("metrics exporter is disabled")]
    ExporterDisabled,

    #[error("counter increment must be non-negative for {name}: {inc}")]
    NegativeCounterIncrement { name: String, inc: i64 },

    #[error(
        "duration histogram {name} is already registered with unit {existing_unit} and description {existing_description:?}; requested unit {requested_unit} and description {requested_description:?}"
    )]
    ConflictingDurationHistogram {
        name: String,
        existing_unit: String,
        existing_description: String,
        requested_unit: String,
        requested_description: String,
    },

    #[error("failed to build OTLP metrics exporter")]
    ExporterBuild {
        #[source]
        source: opentelemetry_otlp::ExporterBuildError,
    },

    #[error("invalid OTLP metrics configuration: {message}")]
    InvalidConfig { message: String },

    #[error("failed to flush or shutdown metrics provider")]
    ProviderShutdown {
        #[source]
        source: opentelemetry_sdk::error::OTelSdkError,
    },

    #[error("runtime metrics snapshot reader is not enabled")]
    RuntimeSnapshotUnavailable,

    #[error("failed to collect runtime metrics snapshot from metrics reader")]
    RuntimeSnapshotCollect {
        #[source]
        source: opentelemetry_sdk::error::OTelSdkError,
    },
}
