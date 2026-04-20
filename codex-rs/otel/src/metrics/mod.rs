mod client;
mod config;
mod error;
pub(crate) mod names;
pub(crate) mod runtime_metrics;
pub(crate) mod tags;
pub(crate) mod timer;
pub(crate) mod validation;

pub use crate::metrics::client::MetricsClient;
pub use crate::metrics::config::MetricsConfig;
pub use crate::metrics::config::MetricsExporter;
pub use crate::metrics::error::MetricsError;
pub use crate::metrics::error::Result;
pub use names::*;
use std::sync::OnceLock;
use std::sync::RwLock;
pub use tags::SessionMetricTagValues;

static GLOBAL_METRICS: OnceLock<RwLock<Option<MetricsClient>>> = OnceLock::new();

fn global_metrics() -> &'static RwLock<Option<MetricsClient>> {
    GLOBAL_METRICS.get_or_init(|| RwLock::new(None))
}

pub(crate) fn replace_global(metrics: Option<MetricsClient>) {
    let mut global = global_metrics()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *global = metrics;
}

pub fn global() -> Option<MetricsClient> {
    global_metrics()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}
