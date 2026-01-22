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
}
