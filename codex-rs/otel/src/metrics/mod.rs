mod batch;
mod client;
mod config;
mod error;
mod tags;
mod time;
mod util;
pub(crate) mod validation;

use std::time::Duration;

pub(crate) const DEFAULT_OTLP_ENDPOINT: &str = "<statsig-otlp-metrics-endpoint>";
pub(crate) const DEFAULT_API_KEY_HEADER: &str = "statsig-api-key";
pub(crate) const DEFAULT_API_KEY: &str = "<statsig-api-key>";
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const DEFAULT_QUEUE_CAPACITY: usize = 1024;
pub(crate) const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);
pub(crate) const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub use crate::metrics::batch::HistogramBuckets;
pub use crate::metrics::batch::MetricsBatch;
pub use crate::metrics::client::MetricsClient;
pub use crate::metrics::config::MetricsConfig;
pub use crate::metrics::error::MetricsError;
pub use crate::metrics::error::Result;

#[cfg(test)]
mod tests;
