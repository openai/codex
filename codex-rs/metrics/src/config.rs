use crate::DEFAULT_TIMEOUT;
use crate::SENTRY_DSN;
use crate::error::Result;
use crate::validation::validate_tag_component;
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct MetricsConfig {
    pub(crate) dsn: String,
    pub(crate) default_tags: BTreeMap<String, String>,
    pub(crate) timeout: Duration,
    pub(crate) user_agent: String,
}

impl MetricsConfig {
    /// Create a config with the provided DSN and default settings.
    pub fn new(dsn: impl Into<String>) -> Self {
        Self {
            dsn: dsn.into(),
            default_tags: BTreeMap::new(),
            timeout: DEFAULT_TIMEOUT,
            user_agent: format!("codex-metrics/{}", env!("CARGO_PKG_VERSION")),
        }
    }

    /// Add a default tag that will be sent with every metric.
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Result<Self> {
        let key = key.into();
        let value = value.into();
        validate_tag_component(&key, "tag key")?;
        validate_tag_component(&value, "tag value")?;
        self.default_tags.insert(key, value);
        Ok(self)
    }

    /// Override the HTTP timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Override the user agent string.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self::new(SENTRY_DSN)
    }
}
