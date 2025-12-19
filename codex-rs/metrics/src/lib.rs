mod batch;
mod client;
mod config;
mod error;
mod statsd;
mod time;
mod util;
mod validation;

use std::time::Duration;

pub(crate) const SENTRY_DSN: &str =
    "https://ae32ed50620d7a7792c1ce5df38b3e3e@o33249.ingest.us.sentry.io/4510195390611458";
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const STATSD_CONTENT_TYPE: &str = "text/plain";
pub(crate) const ENVELOPE_CONTENT_TYPE: &str = "application/x-sentry-envelope";
pub(crate) const DEFAULT_QUEUE_CAPACITY: usize = 1024;
pub(crate) const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);
pub(crate) const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub use crate::batch::HistogramBuckets;
pub use crate::batch::MetricsBatch;
pub use crate::client::MetricsClient;
pub use crate::config::MetricsConfig;
pub use crate::error::MetricsError;
pub use crate::error::Result;
