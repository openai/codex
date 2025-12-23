use crate::config::OtelExporter;
use crate::metrics::Result;
use crate::metrics::validation::validate_tag_key;
use crate::metrics::validation::validate_tag_value;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum MetricsExporter {
    Otlp(OtelExporter),
    InMemory(InMemoryMetricExporter),
}

#[derive(Clone, Debug)]
pub struct MetricsConfig {
    pub(crate) environment: String,
    pub(crate) service_name: String,
    pub(crate) service_version: String,
    pub(crate) exporter: MetricsExporter,
    pub(crate) export_interval: Option<Duration>,
    pub(crate) default_tags: BTreeMap<String, String>,
}

impl MetricsConfig {
    pub fn otlp(
        environment: impl Into<String>,
        service_name: impl Into<String>,
        service_version: impl Into<String>,
        exporter: OtelExporter,
    ) -> Self {
        Self {
            environment: environment.into(),
            service_name: service_name.into(),
            service_version: service_version.into(),
            exporter: MetricsExporter::Otlp(exporter),
            export_interval: None,
            default_tags: BTreeMap::new(),
        }
    }

    /// Create an in-memory config (used in tests).
    pub fn in_memory(
        environment: impl Into<String>,
        service_name: impl Into<String>,
        service_version: impl Into<String>,
        exporter: InMemoryMetricExporter,
    ) -> Self {
        Self {
            environment: environment.into(),
            service_name: service_name.into(),
            service_version: service_version.into(),
            exporter: MetricsExporter::InMemory(exporter),
            export_interval: None,
            default_tags: BTreeMap::new(),
        }
    }

    /// Override the interval between periodic metric exports.
    pub fn with_export_interval(mut self, interval: Duration) -> Self {
        self.export_interval = Some(interval);
        self
    }

    /// Add a default tag that will be sent with every metric.
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Result<Self> {
        let key = key.into();
        let value = value.into();
        validate_tag_key(&key)?;
        validate_tag_value(&value)?;
        self.default_tags.insert(key, value);
        Ok(self)
    }
}
