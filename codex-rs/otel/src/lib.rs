pub mod config;
mod events;
pub mod metrics;
pub mod provider;
pub mod trace_context;

mod otlp;
mod targets;

use crate::metrics::MetricsError;
use crate::metrics::Result as MetricsResult;
use serde::Serialize;
use strum_macros::Display;

pub use crate::events::session_telemetry::SessionTelemetry;
pub use crate::events::session_telemetry::SessionTelemetryMetadata;
pub use crate::metrics::runtime_metrics::RuntimeMetricTotals;
pub use crate::metrics::runtime_metrics::RuntimeMetricsSummary;
pub use crate::metrics::timer::Timer;
pub use crate::provider::OtelProvider;
pub use crate::trace_context::context_from_w3c_trace_context;
pub use crate::trace_context::current_span_trace_id;
pub use crate::trace_context::current_span_w3c_trace_context;
pub use crate::trace_context::set_parent_from_context;
pub use crate::trace_context::set_parent_from_w3c_trace_context;
pub use crate::trace_context::span_w3c_trace_context;
pub use crate::trace_context::traceparent_context_from_env;
pub use codex_utils_string::sanitize_metric_tag_value;

#[derive(Debug, Clone, Serialize, Display)]
#[serde(rename_all = "snake_case")]
pub enum ToolDecisionSource {
    Config,
    User,
}

/// Maps to core AuthMode to avoid a circular dependency on codex-core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum TelemetryAuthMode {
    ApiKey,
    Chatgpt,
}

/// Start a metrics timer using the globally installed metrics client.
pub fn start_global_timer(name: &str, tags: &[(&str, &str)]) -> MetricsResult<Timer> {
    let Some(metrics) = crate::metrics::global() else {
        return Err(MetricsError::ExporterDisabled);
    };
    metrics.start_timer(name, tags)
}
