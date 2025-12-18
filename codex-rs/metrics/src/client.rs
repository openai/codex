use crate::batch::HistogramBuckets;
use crate::batch::MetricsBatch;
use crate::config::MetricsConfig;
use crate::error::MetricsError;
use crate::error::Result;
use crate::statsd::build_statsd_envelope;
use crate::validation::validate_tags;
use sentry::types::Dsn;
use std::collections::BTreeMap;

const ENVELOPE_CONTENT_TYPE: &str = "application/x-sentry-envelope";

#[derive(Debug)]
pub struct MetricsClient {
    dsn: Dsn,
    http: reqwest::blocking::Client,
    auth_header: String,
    default_tags: BTreeMap<String, String>,
}

impl MetricsClient {
    /// Build a metrics client from configuration and validate defaults.
    pub fn new(config: MetricsConfig) -> Result<Self> {
        let dsn_value = config.dsn.clone();
        let dsn = dsn_value
            .parse::<Dsn>()
            .map_err(|source| MetricsError::InvalidDsn {
                dsn: dsn_value,
                source,
            })?;
        validate_tags(&config.default_tags)?;

        let http = reqwest::blocking::Client::builder()
            .timeout(config.timeout)
            .user_agent(config.user_agent.clone())
            .build()
            .map_err(|source| MetricsError::HttpClientBuild { source })?;

        let auth_header = dsn.to_auth(Some(&config.user_agent)).to_string();

        Ok(Self {
            dsn,
            http,
            auth_header,
            default_tags: config.default_tags,
        })
    }

    /// Send a single counter increment.
    pub fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) -> Result<()> {
        let mut batch = MetricsBatch::new();
        batch.counter(name, inc, tags)?;
        self.send(batch)
    }

    /// Send a single histogram sample with the provided buckets.
    pub fn histogram(
        &self,
        name: &str,
        value: i64,
        buckets: &HistogramBuckets,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        let mut batch = MetricsBatch::new();
        batch.histogram(name, value, buckets, tags)?;
        self.send(batch)
    }

    /// Create an empty batch for multi-metric sends.
    pub fn batch(&self) -> MetricsBatch {
        MetricsBatch::new()
    }

    /// Send a batch of metrics to Sentry (no-op if the batch is empty).
    pub fn send(&self, batch: MetricsBatch) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let payload = batch.render(&self.default_tags)?;
        let envelope = build_statsd_envelope(&self.dsn, &payload)?;

        let response = self
            .http
            .post(self.dsn.envelope_api_url())
            .header("X-Sentry-Auth", &self.auth_header)
            .header("Content-Type", ENVELOPE_CONTENT_TYPE)
            .body(envelope)
            .send()
            .map_err(|source| MetricsError::SendEnvelope { source })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .map(|body| {
                    if body.is_empty() {
                        String::new()
                    } else {
                        format!(" body: {body}")
                    }
                })
                .unwrap_or_default();
            return Err(MetricsError::SentryUploadFailed { status, body });
        }

        Ok(())
    }
}
