mod client;
mod config;
mod error;
mod event;
mod sink;
mod tags;
mod time;
mod util;
pub(crate) mod validation;
mod worker;

use std::time::Duration;

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const DEFAULT_QUEUE_CAPACITY: usize = 1024;
pub(crate) const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);
pub(crate) const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub use crate::metrics::client::MetricsClient;
pub use crate::metrics::config::MetricsConfig;
pub use crate::metrics::error::MetricsError;
pub use crate::metrics::error::Result;
pub(crate) use crate::metrics::event::MetricEvent;
