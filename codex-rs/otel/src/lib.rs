pub mod config;
pub mod metrics;
pub mod otel_provider;
pub mod traces;

mod otlp;

use crate::metrics::MetricsClient;
use crate::metrics::MetricsConfig;
use crate::metrics::MetricsError;
use crate::metrics::Result as MetricsResult;
use crate::metrics::timer::Timer;
use crate::metrics::validation::validate_tag_key;
use crate::metrics::validation::validate_tag_value;
use crate::otel_provider::OtelProvider;
use codex_protocol::ThreadId;
use serde::Serialize;
use std::time::Duration;
use strum_macros::Display;

#[derive(Debug, Clone, Serialize, Display)]
#[serde(rename_all = "snake_case")]
pub enum ToolDecisionSource {
    Config,
    User,
}

#[derive(Debug, Clone)]
pub struct OtelEventMetadata {
    pub(crate) conversation_id: ThreadId,
    pub(crate) auth_mode: Option<String>,
    pub(crate) account_id: Option<String>,
    pub(crate) account_email: Option<String>,
    pub(crate) model: String,
    pub(crate) slug: String,
    pub(crate) log_user_prompts: bool,
    pub(crate) app_version: &'static str,
    pub(crate) terminal_type: String,
}

#[derive(Debug, Clone)]
pub struct OtelManager {
    pub(crate) metadata: OtelEventMetadata,
    pub(crate) metrics: Option<MetricsClient>,
    pub(crate) metrics_use_metadata_tags: bool,
}

impl OtelManager {
    pub fn with_model(mut self, model: &str, slug: &str) -> Self {
        self.metadata.model = model.to_owned();
        self.metadata.slug = slug.to_owned();
        self
    }

    pub fn with_metrics(mut self, metrics: MetricsClient) -> Self {
        self.metrics = Some(metrics);
        self.metrics_use_metadata_tags = true;
        self
    }

    pub fn with_metrics_without_metadata_tags(mut self, metrics: MetricsClient) -> Self {
        self.metrics = Some(metrics);
        self.metrics_use_metadata_tags = false;
        self
    }

    pub fn with_metrics_config(self, config: MetricsConfig) -> MetricsResult<Self> {
        let metrics = MetricsClient::new(config)?;
        Ok(self.with_metrics(metrics))
    }

    pub fn with_provider_metrics(self, provider: &OtelProvider) -> Self {
        match provider.metrics() {
            Some(metrics) => self.with_metrics(metrics.clone()),
            None => self,
        }
    }

    pub fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) {
        let res: MetricsResult<()> = (|| {
            let Some(metrics) = &self.metrics else {
                return Ok(());
            };

            let tags = self.tags_with_metadata(tags)?;
            metrics.counter(name, inc, &tags)
        })();

        if let Err(e) = res {
            tracing::warn!("metrics counter [{name}] failed: {e}");
        }
    }

    pub fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) {
        let res: MetricsResult<()> = (|| {
            let Some(metrics) = &self.metrics else {
                return Ok(());
            };

            let tags = self.tags_with_metadata(tags)?;
            metrics.histogram(name, value, &tags)
        })();

        if let Err(e) = res {
            tracing::warn!("metrics histogram [{name}] failed: {e}");
        }
    }

    pub fn record_duration(&self, name: &str, duration: Duration, tags: &[(&str, &str)]) {
        let res: MetricsResult<()> = (|| {
            let Some(metrics) = &self.metrics else {
                return Ok(());
            };

            let tags = self.tags_with_metadata(tags)?;
            metrics.record_duration(name, duration, &tags)
        })();

        if let Err(e) = res {
            tracing::warn!("metrics duration [{name}] failed: {e}");
        }
    }

    pub fn start_timer(&self, name: &str, tags: &[(&str, &str)]) -> Result<Timer, MetricsError> {
        let Some(metrics) = &self.metrics else {
            return Err(MetricsError::ExporterDisabled);
        };
        let tags = self.tags_with_metadata(tags)?;
        metrics.start_timer(name, &tags)
    }

    pub fn shutdown_metrics(&self) -> MetricsResult<()> {
        let Some(metrics) = &self.metrics else {
            return Ok(());
        };
        metrics.shutdown()
    }

    fn tags_with_metadata<'a>(
        &'a self,
        tags: &'a [(&'a str, &'a str)],
    ) -> MetricsResult<Vec<(&'a str, &'a str)>> {
        let mut merged = self.metadata_tag_refs()?;
        merged.extend(tags.iter().copied());
        Ok(merged)
    }

    fn metadata_tag_refs(&self) -> MetricsResult<Vec<(&str, &str)>> {
        if !self.metrics_use_metadata_tags {
            return Ok(Vec::new());
        }
        let mut tags = Vec::with_capacity(5);
        Self::push_metadata_tag(&mut tags, "auth_mode", self.metadata.auth_mode.as_deref())?;
        Self::push_metadata_tag(&mut tags, "model", Some(self.metadata.model.as_str()))?;
        Self::push_metadata_tag(&mut tags, "app.version", Some(self.metadata.app_version))?;
        Ok(tags)
    }

    fn push_metadata_tag<'a>(
        tags: &mut Vec<(&'a str, &'a str)>,
        key: &'static str,
        value: Option<&'a str>,
    ) -> MetricsResult<()> {
        let Some(value) = value else {
            return Ok(());
        };
        validate_tag_key(key)?;
        validate_tag_value(value)?;
        tags.push((key, value));
        Ok(())
    }
}
