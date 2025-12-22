use crate::metrics::MetricEvent;
use crate::metrics::MetricsError;
use crate::metrics::sink::MetricSink;
use chrono::Utc;
use http::HeaderName;
use http::HeaderValue;
use http::header::USER_AGENT;
use reqwest::Client;
use serde::Serialize;
use std::collections::BTreeMap;
use std::pin::Pin;
use std::time::Duration;

// Publicly available API key for codex local project.
pub(crate) const DEFAULT_API_KEY: &str = "client-MkRuleRQBd6qakfnDYqJVR9JuXcY57Ljly3vi5JVUIO";
pub(crate) const DEFAULT_STATSIG_ENDPOINT: &str = "https://ab.chatgpt.com/v1/log_event";
pub(crate) const DEFAULT_API_KEY_HEADER: &str = "statsig-api-key";
const STATSIG_USER_ID: &str = "codex-metrics";
const STATSIG_SDK_TYPE: &str = "codex-otel-rust";

pub(crate) struct StatsigExporter {
    client: Client,
    endpoint: String,
    api_key_header: HeaderName,
    api_key: HeaderValue,
    user_agent: Option<HeaderValue>,
}

impl StatsigExporter {
    pub(crate) fn from(
        endpoint: &str,
        api_key_header: &str,
        timeout: &Duration,
        user_agent: &str,
        api_key: &str,
    ) -> crate::metrics::Result<Self> {
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
        let client = Client::builder()
            .timeout(*timeout)
            .build()
            .map_err(|source| MetricsError::HttpClientBuild { source })?;

        Ok(Self {
            client,
            endpoint: endpoint.to_string(),
            api_key_header,
            api_key,
            user_agent,
        })
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
                    tags,
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
                    tags,
                },
                user: StatsigUser {
                    user_id: STATSIG_USER_ID.to_string(),
                },
                time: timestamp,
            },
        }
    }
}

impl MetricSink for StatsigExporter {
    fn export_batch<'a>(
        &'a mut self,
        events: Vec<MetricEvent>,
    ) -> Pin<Box<dyn Future<Output = crate::metrics::Result<()>> + Send + 'a>> {
        Box::pin(async move {
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
        })
    }

    fn shutdown<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = crate::metrics::Result<()>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
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
