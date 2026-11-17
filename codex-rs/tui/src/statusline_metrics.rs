//! Aggregate statusline counters with fixed labels and the current analytics consent.

use crate::chatwidget::ChatWidget;
use crate::legacy_core::config::Config;
use codex_config::types::DEFAULT_OTEL_ENVIRONMENT;
use codex_config::types::OtelExporterKind;
use codex_features::Feature;
use codex_otel::MetricsClient;

#[derive(Default)]
pub(crate) struct StatuslineMetrics {
    first_prompt_recorded: bool,
    collector: Option<(MetricsClient, OtelExporterKind)>,
}

impl StatuslineMetrics {
    pub(crate) fn new(config: &Config, metrics: Option<MetricsClient>, originator: &str) -> Self {
        Self::with_env(config, metrics, originator, |key| std::env::var_os(key))
    }

    fn with_env(
        config: &Config,
        metrics: Option<MetricsClient>,
        originator: &str,
        get: impl Fn(&str) -> Option<std::ffi::OsString>,
    ) -> Self {
        let exporter = &config.otel.metrics_exporter;
        // The exporter captures resource attributes and headers at launch. Custom values may
        // identify the installation and belong only in an explicitly configured collector.
        let eligible = match exporter {
            OtelExporterKind::None => false,
            OtelExporterKind::Statsig => {
                config.otel.environment == DEFAULT_OTEL_ENVIRONMENT
                    && codex_otel::bounded_originator_tag_value(originator) == originator
                    && [
                        "OTEL_RESOURCE_ATTRIBUTES",
                        "OTEL_EXPORTER_OTLP_METRICS_HEADERS",
                        "OTEL_EXPORTER_OTLP_HEADERS",
                    ]
                    .into_iter()
                    .all(|key| get(key).is_none_or(|value| value.is_empty()))
            }
            OtelExporterKind::OtlpHttp { .. } | OtelExporterKind::OtlpGrpc { .. } => true,
        };
        Self {
            first_prompt_recorded: false,
            collector: metrics
                .filter(|_| eligible)
                .map(|client| (client, exporter.clone())),
        }
    }

    pub(crate) fn begin_visit(&mut self) {
        self.first_prompt_recorded = false;
    }

    pub(crate) fn record_first_prompt(&mut self, config: &Config, chat: &ChatWidget) {
        if self.first_prompt_recorded {
            return;
        }
        self.first_prompt_recorded = true;
        let mode = if !chat
            .config_ref()
            .features
            .enabled(Feature::TuiExtensionsStatusLine)
        {
            "feature_off"
        } else if chat.selected_statusline_provider().is_some() {
            "custom"
        } else {
            "builtin"
        };
        self.record(config, "codex.statusline.first_prompt", &[("mode", mode)]);
    }

    pub(crate) fn record(
        &self,
        config: &Config,
        name: &'static str,
        labels: &[(&'static str, &'static str)],
    ) {
        if let Some((client, exporter)) = &self.collector
            && exporter == &config.otel.metrics_exporter
        {
            let _ = client.counter_with_analytics_consent(
                name,
                /*inc*/ 1,
                labels,
                config.analytics_enabled != Some(false),
            );
        }
    }
}

#[cfg(test)]
#[path = "statusline_metrics_tests.rs"]
pub(crate) mod tests;
