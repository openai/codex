use crate::metrics::config::MetricsConfig;
use crate::metrics::config::MetricsExporter;
use crate::metrics::error::MetricsError;
use crate::metrics::error::Result;
use crate::metrics::tags::merge_tags;
use crate::metrics::util::error_or_panic;
use chrono::Utc;
use opentelemetry::KeyValue;
use opentelemetry::metrics::Histogram;
use opentelemetry::metrics::Meter;
use opentelemetry::metrics::MeterProvider;
use opentelemetry::metrics::UpDownCounter;
use opentelemetry_sdk::metrics::PeriodicReader;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::header::USER_AGENT;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::time::Duration;

pub(crate) const METER_NAME: &str = "codex-otel-metrics";
const STATSIG_USER_ID: &str = "codex-metrics";
const STATSIG_SDK_TYPE: &str = "codex-otel-rust";

#[derive(Clone, Debug)]
pub(crate) enum MetricEvent {
    Counter {
        name: String,
        value: i64,
        tags: Vec<(String, String)>,
    },
    Histogram {
        name: String,
        value: i64,
        tags: Vec<(String, String)>,
    },
}

pub(crate) fn build_worker_exporter(config: &MetricsConfig) -> Result<WorkerExporter> {
    match &config.exporter {
        MetricsExporter::StatsigHttp {
            endpoint,
            api_key_header,
            timeout,
            user_agent,
        } => Ok(WorkerExporter::Statsig(StatsigExporter::from(
            endpoint,
            api_key_header,
            timeout,
            user_agent,
            &config.api_key,
            &config.default_tags,
        )?)),
        MetricsExporter::InMemory(exporter) => Ok(WorkerExporter::InMemory(
            InMemoryExporter::from(config.default_tags.clone(), exporter.clone()),
        )),
    }
}

pub(crate) enum WorkerExporter {
    Statsig(StatsigExporter),
    InMemory(InMemoryExporter),
}

pub(crate) struct InMemoryExporter {
    recorder: MetricRecorder,
    meter_provider: SdkMeterProvider,
}

impl InMemoryExporter {
    fn from(
        default_tags: BTreeMap<String, String>,
        exporter: opentelemetry_sdk::metrics::InMemoryMetricExporter,
    ) -> Self {
        let reader = PeriodicReader::builder(exporter).build();
        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let meter = meter_provider.meter(METER_NAME);
        let recorder = MetricRecorder::new(meter, default_tags);
        Self {
            recorder,
            meter_provider,
        }
    }

    pub(crate) async fn export_events(&mut self, events: Vec<MetricEvent>, exporter_label: &str) {
        for event in events {
            self.recorder.record_event(event);
        }
        if let Err(err) = self.meter_provider.force_flush() {
            error_or_panic(format!(
                "metrics flush failed: {err} (exporter={exporter_label})"
            ));
        }
    }

    pub(crate) async fn shutdown(&mut self, exporter_label: &str) {
        if let Err(err) = self.meter_provider.force_flush() {
            error_or_panic(format!(
                "metrics flush failed during shutdown: {err} (exporter={exporter_label})"
            ));
        }
        if let Err(err) = self.meter_provider.shutdown() {
            error_or_panic(format!(
                "metrics shutdown failed: {err} (exporter={exporter_label})"
            ));
        }
    }
}

#[derive(Debug)]
struct MetricRecorder {
    meter: Meter,
    counters: HashMap<String, UpDownCounter<i64>>,
    histograms: HashMap<String, Histogram<f64>>,
    default_tags: BTreeMap<String, String>,
}

impl MetricRecorder {
    fn new(meter: Meter, default_tags: BTreeMap<String, String>) -> Self {
        Self {
            meter,
            counters: HashMap::new(),
            histograms: HashMap::new(),
            default_tags,
        }
    }

    fn record_event(&mut self, event: MetricEvent) {
        match event {
            MetricEvent::Counter { name, value, tags } => {
                self.record_counter(&name, value, &tags);
            }
            MetricEvent::Histogram { name, value, tags } => {
                self.record_histogram(&name, value, &tags);
            }
        }
    }

    fn record_counter(&mut self, name: &str, value: i64, tags: &[(String, String)]) {
        let attributes = self.attributes_for(tags);
        let name = name.to_string();
        let counter = self
            .counters
            .entry(name.clone())
            .or_insert_with(|| self.meter.i64_up_down_counter(name.clone()).build());
        counter.add(value, &attributes);
    }

    fn record_histogram(&mut self, name: &str, value: i64, tags: &[(String, String)]) {
        let attributes = self.attributes_for(tags);
        let name = name.to_string();
        let histogram = self
            .histograms
            .entry(name.clone())
            .or_insert_with(|| self.meter.f64_histogram(name.clone()).build());
        histogram.record(value as f64, &attributes);
    }

    fn attributes_for(&self, tags: &[(String, String)]) -> Vec<KeyValue> {
        let merged = merge_tags(&self.default_tags, tags);
        merged
            .iter()
            .map(|(key, value)| KeyValue::new(key.clone(), value.clone()))
            .collect()
    }
}

pub(crate) struct StatsigExporter {
    client: reqwest::Client,
    endpoint: String,
    api_key_header: HeaderName,
    api_key: HeaderValue,
    user_agent: Option<HeaderValue>,
    default_tags: BTreeMap<String, String>,
}

impl StatsigExporter {
    fn from(
        endpoint: &str,
        api_key_header: &str,
        timeout: &Duration,
        user_agent: &str,
        api_key: &str,
        default_tags: &BTreeMap<String, String>,
    ) -> Result<Self> {
        let api_key_header =
            HeaderName::from_bytes(api_key_header.as_bytes()).map_err(|source| {
                MetricsError::InvalidApiKeyHeader {
                    header: api_key_header.to_string(),
                    source,
                }
            })?;
        let api_key =
            HeaderValue::from_str(api_key).map_err(|source| MetricsError::InvalidHeaderValue {
                header: api_key_header.to_string(),
                source,
            })?;
        let user_agent = if user_agent.is_empty() {
            None
        } else {
            Some(HeaderValue::from_str(user_agent).map_err(|source| {
                MetricsError::InvalidHeaderValue {
                    header: "User-Agent".to_string(),
                    source,
                }
            })?)
        };
        let client = reqwest::Client::builder()
            .timeout(*timeout)
            .build()
            .map_err(|source| MetricsError::HttpClientBuild { source })?;

        Ok(Self {
            client,
            endpoint: endpoint.to_string(),
            api_key_header,
            api_key,
            user_agent,
            default_tags: default_tags.clone(),
        })
    }

    pub(crate) async fn export_events(&self, events: Vec<MetricEvent>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let payload = self.build_payload(events);

        let mut request = self
            .client
            .post(&self.endpoint)
            .header(self.api_key_header.clone(), self.api_key.clone());

        if let Some(user_agent) = &self.user_agent {
            request = request.header(USER_AGENT, user_agent.clone());
        }

        let response = request
            .json(&payload)
            .send()
            .await
            .map_err(|source| MetricsError::StatsigRequestFailed { source })?;

        if let Err(status_err) = response.error_for_status_ref() {
            let status = status_err
                .status()
                .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR);
            let body = response.text().await.unwrap_or_default();
            return Err(MetricsError::StatsigResponseError { status, body });
        }

        Ok(())
    }

    fn build_payload(&self, events: Vec<MetricEvent>) -> StatsigPayload {
        let timestamp = Utc::now().timestamp_millis();
        let events = events
            .into_iter()
            .map(|event| self.event_from_metric(event, timestamp))
            .collect();

        StatsigPayload {
            events,
            statsig_metadata: StatsigMetadata {
                sdk_type: STATSIG_SDK_TYPE.to_string(),
                sdk_version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }
    }

    fn event_from_metric(&self, event: MetricEvent, timestamp: i64) -> StatsigEvent {
        match event {
            MetricEvent::Counter { name, value, tags } => StatsigEvent {
                event_name: name,
                value: value as f64,
                metadata: StatsigEventMetadata {
                    metric_type: "counter".to_string(),
                    tags: merge_tags(&self.default_tags, &tags),
                },
                user: StatsigUser {
                    user_id: STATSIG_USER_ID.to_string(),
                },
                time: timestamp,
            },
            MetricEvent::Histogram { name, value, tags } => StatsigEvent {
                event_name: name,
                value: value as f64,
                metadata: StatsigEventMetadata {
                    metric_type: "histogram".to_string(),
                    tags: merge_tags(&self.default_tags, &tags),
                },
                user: StatsigUser {
                    user_id: STATSIG_USER_ID.to_string(),
                },
                time: timestamp,
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct StatsigPayload {
    events: Vec<StatsigEvent>,
    #[serde(rename = "statsigMetadata")]
    statsig_metadata: StatsigMetadata,
}

#[derive(Debug, Serialize)]
struct StatsigEvent {
    #[serde(rename = "eventName")]
    event_name: String,
    value: f64,
    metadata: StatsigEventMetadata,
    user: StatsigUser,
    time: i64,
}

#[derive(Debug, Serialize)]
struct StatsigEventMetadata {
    #[serde(rename = "metric_type")]
    metric_type: String,
    #[serde(flatten)]
    tags: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct StatsigUser {
    #[serde(rename = "userID")]
    user_id: String,
}

#[derive(Debug, Serialize)]
struct StatsigMetadata {
    #[serde(rename = "sdkType")]
    sdk_type: String,
    #[serde(rename = "sdkVersion")]
    sdk_version: String,
}
