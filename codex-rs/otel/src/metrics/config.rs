use crate::config::OtelExporter;
use crate::metrics::Result;
use crate::metrics::names::API_CALL_COUNT_METRIC;
use crate::metrics::names::API_CALL_DURATION_METRIC;
use crate::metrics::names::RESPONSES_API_ENGINE_IAPI_TTFT_DURATION_METRIC;
use crate::metrics::names::RESPONSES_API_ENGINE_SERVICE_TBT_DURATION_METRIC;
use crate::metrics::names::RESPONSES_API_ENGINE_SERVICE_TTFT_DURATION_METRIC;
use crate::metrics::names::TOOL_CALL_COUNT_METRIC;
use crate::metrics::names::TOOL_CALL_DURATION_METRIC;
use crate::metrics::names::TURN_TOKEN_USAGE_METRIC;
use crate::metrics::validation::validate_tag_key;
use crate::metrics::validation::validate_tag_value;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use std::collections::BTreeMap;
use std::time::Duration;

const CONVERSATION_TURN_COUNT_METRIC: &str = "codex.conversation.turn.count";

// Metrics intentionally not sent through Codex's built-in Statsig route.
// Keep this as an exact-name list so custom OTLP exporters still receive them.
const STATSIG_DISABLED_METRICS: &[&str] = &[
    API_CALL_COUNT_METRIC,
    API_CALL_DURATION_METRIC,
    CONVERSATION_TURN_COUNT_METRIC,
    RESPONSES_API_ENGINE_IAPI_TTFT_DURATION_METRIC,
    RESPONSES_API_ENGINE_SERVICE_TBT_DURATION_METRIC,
    RESPONSES_API_ENGINE_SERVICE_TTFT_DURATION_METRIC,
    TOOL_CALL_COUNT_METRIC,
    TOOL_CALL_DURATION_METRIC,
    TURN_TOKEN_USAGE_METRIC,
];

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
    pub(crate) runtime_reader: bool,
    pub(crate) statsig_disabled_metrics: &'static [&'static str],
    pub(crate) default_tags: BTreeMap<String, String>,
}

impl MetricsConfig {
    pub fn otlp(
        environment: impl Into<String>,
        service_name: impl Into<String>,
        service_version: impl Into<String>,
        exporter: OtelExporter,
    ) -> Self {
        let statsig_disabled_metrics = if matches!(exporter, OtelExporter::Statsig) {
            STATSIG_DISABLED_METRICS
        } else {
            &[]
        };
        Self {
            environment: environment.into(),
            service_name: service_name.into(),
            service_version: service_version.into(),
            exporter: MetricsExporter::Otlp(exporter),
            export_interval: None,
            runtime_reader: false,
            statsig_disabled_metrics,
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
            runtime_reader: false,
            statsig_disabled_metrics: &[],
            default_tags: BTreeMap::new(),
        }
    }

    /// Override the interval between periodic metric exports.
    pub fn with_export_interval(mut self, interval: Duration) -> Self {
        self.export_interval = Some(interval);
        self
    }

    /// Enable a manual reader for on-demand runtime snapshots.
    pub fn with_runtime_reader(mut self) -> Self {
        self.runtime_reader = true;
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
