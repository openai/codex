mod client;
mod config;
mod error;
pub(crate) mod names;
pub(crate) mod runtime_metrics;
pub(crate) mod tags;
pub(crate) mod timer;
pub(crate) mod validation;

use crate::config::OtelMetricsSettings;
pub use crate::metrics::client::MetricsClient;
pub use crate::metrics::config::MetricsConfig;
pub use crate::metrics::config::MetricsExporter;
pub use crate::metrics::error::MetricsError;
pub use crate::metrics::error::Result;
pub use names::*;
use std::sync::OnceLock;
pub use tags::SessionMetricTagValues;

static GLOBAL_METRICS: OnceLock<MetricsClient> = OnceLock::new();
static GLOBAL_METRICS_SETTINGS: OnceLock<OtelMetricsSettings> = OnceLock::new();

pub(crate) fn install_global(metrics: MetricsClient, settings: OtelMetricsSettings) {
    let _ = GLOBAL_METRICS.set(metrics);
    let _ = GLOBAL_METRICS_SETTINGS.set(settings);
}

pub fn global() -> Option<MetricsClient> {
    GLOBAL_METRICS.get().cloned()
}

pub(crate) fn global_settings() -> Option<OtelMetricsSettings> {
    GLOBAL_METRICS_SETTINGS.get().cloned()
}
