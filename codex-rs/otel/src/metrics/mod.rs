mod client;
mod config;
mod error;
mod exporter;
mod tags;
mod time;
mod util;
pub(crate) mod validation;
mod worker;

use std::time::Duration;

// Publicly available API key for codex local project.
pub(crate) const DEFAULT_API_KEY: &str = "client-MkRuleRQBd6qakfnDYqJVR9JuXcY57Ljly3vi5JVUIO";
pub(crate) const DEFAULT_STATSIG_ENDPOINT: &str = "https://ab.chatgpt.com/v1/log_event";
pub(crate) const DEFAULT_API_KEY_HEADER: &str = "statsig-api-key";

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const DEFAULT_QUEUE_CAPACITY: usize = 1024;
pub(crate) const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);
pub(crate) const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub use crate::metrics::client::MetricsClient;
pub use crate::metrics::config::MetricsConfig;
pub use crate::metrics::error::MetricsError;
pub use crate::metrics::error::Result;
