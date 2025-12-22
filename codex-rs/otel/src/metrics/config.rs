use crate::metrics::DEFAULT_TIMEOUT;
use crate::metrics::error::Result;
use crate::metrics::sink::statsig::DEFAULT_API_KEY;
use crate::metrics::sink::statsig::DEFAULT_API_KEY_HEADER;
use crate::metrics::sink::statsig::DEFAULT_STATSIG_ENDPOINT;
use crate::metrics::validation::validate_tag_key;
use crate::metrics::validation::validate_tag_value;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Debug)]
pub(crate) enum MetricsExporter {
    StatsigHttp {
        endpoint: String,
        api_key_header: String,
        timeout: Duration,
        user_agent: String,
    },
    InMemory(InMemoryMetricExporter),
}

impl MetricsExporter {
    pub(crate) fn statsig_defaults() -> Self {
        Self::StatsigHttp {
            endpoint: DEFAULT_STATSIG_ENDPOINT.to_string(),
            api_key_header: DEFAULT_API_KEY_HEADER.to_string(),
            timeout: DEFAULT_TIMEOUT,
            user_agent: format!("codex-otel-metrics/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MetricsConfig {
    pub(crate) api_key: String,
    pub(crate) default_tags: BTreeMap<String, String>,
    pub(crate) exporter: MetricsExporter,
}

impl MetricsConfig {
    /// Create a Statsig config with the provided API key and default settings.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::statsig(api_key)
    }

    /// Create a Statsig config with the provided API key and default settings.
    pub fn statsig(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            default_tags: BTreeMap::new(),
            exporter: MetricsExporter::statsig_defaults(),
        }
    }

    /// Create an in-memory config (used in tests).
    pub fn in_memory(exporter: opentelemetry_sdk::metrics::InMemoryMetricExporter) -> Self {
        Self {
            api_key: String::new(),
            default_tags: BTreeMap::new(),
            exporter: MetricsExporter::InMemory(exporter),
        }
    }

    /// Override the Statsig endpoint.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        if let MetricsExporter::StatsigHttp { endpoint: e, .. } = &mut self.exporter {
            *e = endpoint.into();
        }
        self
    }

    /// Override the API key header name.
    pub fn with_api_key_header(mut self, header: impl Into<String>) -> Self {
        if let MetricsExporter::StatsigHttp { api_key_header, .. } = &mut self.exporter {
            *api_key_header = header.into();
        }
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
        if let MetricsExporter::StatsigHttp { timeout: t, .. } = &mut self.exporter {
            *t = timeout;
        }
        self
    }

    /// Override the HTTP user agent header.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        if let MetricsExporter::StatsigHttp { user_agent: ua, .. } = &mut self.exporter {
            *ua = user_agent.into();
        }
        self
    }

    pub(crate) fn exporter_label(&self) -> String {
        match &self.exporter {
            MetricsExporter::StatsigHttp {
                endpoint, timeout, ..
            } => format!("statsig_http endpoint={endpoint} timeout={timeout:?}"),
            MetricsExporter::InMemory(_) => "in_memory".to_string(),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        // `cfg(test)` only applies to *unit tests* within this crate. Integration tests compile
        // `codex-otel` as a normal dependency, so they must opt into the in-memory default via a
        // feature (see `test-in-memory-metrics`).
        if cfg!(any(test, feature = "test-in-memory-metrics")) {
            Self::in_memory(InMemoryMetricExporter::default())
        } else {
            Self::statsig(DEFAULT_API_KEY)
        }
    }
}
