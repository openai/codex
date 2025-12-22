use crate::metrics::DEFAULT_API_KEY;
use crate::metrics::DEFAULT_API_KEY_HEADER;
use crate::metrics::DEFAULT_STATSIG_ENDPOINT;
use crate::metrics::DEFAULT_TIMEOUT;
use crate::metrics::error::Result;
use crate::metrics::validation::validate_tag_key;
use crate::metrics::validation::validate_tag_value;
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Debug)]
pub(crate) enum MetricsExporter {
    StatsigHttp,
    InMemory(opentelemetry_sdk::metrics::InMemoryMetricExporter),
}

#[derive(Clone, Debug)]
pub struct MetricsConfig {
    pub(crate) endpoint: String,
    pub(crate) api_key: String,
    pub(crate) api_key_header: String,
    pub(crate) default_tags: BTreeMap<String, String>,
    pub(crate) timeout: Duration,
    pub(crate) user_agent: String,
    pub(crate) exporter: MetricsExporter,
}

impl MetricsConfig {
    /// Create a config with the provided API key and default settings.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            endpoint: DEFAULT_STATSIG_ENDPOINT.to_string(),
            api_key: api_key.into(),
            api_key_header: DEFAULT_API_KEY_HEADER.to_string(),
            default_tags: BTreeMap::new(),
            timeout: DEFAULT_TIMEOUT,
            user_agent: format!("codex-otel-metrics/{}", env!("CARGO_PKG_VERSION")),
            exporter: MetricsExporter::StatsigHttp,
        }
    }

    /// Override the Statsig endpoint.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Override the API key header name.
    pub fn with_api_key_header(mut self, header: impl Into<String>) -> Self {
        self.api_key_header = header.into();
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

    /// Override the HTTP client timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Override the HTTP user agent header.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    pub fn with_in_memory_exporter(
        mut self,
        exporter: opentelemetry_sdk::metrics::InMemoryMetricExporter,
    ) -> Self {
        self.exporter = MetricsExporter::InMemory(exporter);
        self
    }

    pub(crate) fn exporter_label(&self) -> String {
        match &self.exporter {
            MetricsExporter::StatsigHttp => {
                format!(
                    "statsig_http endpoint={} timeout={:?}",
                    self.endpoint, self.timeout
                )
            }
            MetricsExporter::InMemory(_) => "in_memory".to_string(),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self::new(DEFAULT_API_KEY)
    }
}
