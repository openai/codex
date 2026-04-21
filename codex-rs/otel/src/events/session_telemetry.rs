use crate::TelemetryAuthMode;
use crate::ToolDecisionSource;
use crate::events::shared::log_and_trace_event;
use crate::events::shared::log_event;
use crate::events::shared::trace_event;
use crate::metrics::API_CALL_COUNT_METRIC;
use crate::metrics::API_CALL_DURATION_METRIC;
use crate::metrics::MetricsClient;
use crate::metrics::MetricsConfig;
use crate::metrics::MetricsError;
use crate::metrics::PROFILE_USAGE_METRIC;
use crate::metrics::RESPONSES_API_ENGINE_IAPI_TBT_DURATION_METRIC;
use crate::metrics::RESPONSES_API_ENGINE_IAPI_TTFT_DURATION_METRIC;
use crate::metrics::RESPONSES_API_ENGINE_SERVICE_TBT_DURATION_METRIC;
use crate::metrics::RESPONSES_API_ENGINE_SERVICE_TTFT_DURATION_METRIC;
use crate::metrics::RESPONSES_API_INFERENCE_TIME_DURATION_METRIC;
use crate::metrics::RESPONSES_API_OVERHEAD_DURATION_METRIC;
use crate::metrics::Result as MetricsResult;
use crate::metrics::SSE_EVENT_COUNT_METRIC;
use crate::metrics::SSE_EVENT_DURATION_METRIC;
use crate::metrics::SessionMetricTagValues;
use crate::metrics::TOOL_CALL_COUNT_METRIC;
use crate::metrics::TOOL_CALL_DURATION_METRIC;
use crate::metrics::WEBSOCKET_EVENT_COUNT_METRIC;
use crate::metrics::WEBSOCKET_EVENT_DURATION_METRIC;
use crate::metrics::WEBSOCKET_REQUEST_COUNT_METRIC;
use crate::metrics::WEBSOCKET_REQUEST_DURATION_METRIC;
use crate::metrics::runtime_metrics::RuntimeMetricsSummary;
use crate::metrics::timer::Timer;
use crate::provider::OtelProvider;
use crate::sanitize_metric_tag_value;
use codex_api::ApiError;
use codex_api::ResponseEvent;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use eventsource_stream::Event as StreamEvent;
use eventsource_stream::EventStreamError as StreamError;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use reqwest::Error;
use reqwest::Response;
use std::borrow::Cow;
use std::future::Future;
use std::time::Duration;
use std::time::Instant;
use tokio::time::error::Elapsed;
use tracing::Span;

const SSE_UNKNOWN_KIND: &str = "unknown";
const WEBSOCKET_UNKNOWN_KIND: &str = "unknown";
const RESPONSES_WEBSOCKET_TIMING_KIND: &str = "responsesapi.websocket_timing";
const RESPONSES_WEBSOCKET_TIMING_METRICS_FIELD: &str = "timing_metrics";
const RESPONSES_API_OVERHEAD_FIELD: &str = "responses_duration_excl_engine_and_client_tool_time_ms";
const RESPONSES_API_INFERENCE_FIELD: &str = "engine_service_total_ms";
const RESPONSES_API_ENGINE_IAPI_TTFT_FIELD: &str = "engine_iapi_ttft_total_ms";
const RESPONSES_API_ENGINE_SERVICE_TTFT_FIELD: &str = "engine_service_ttft_total_ms";
const RESPONSES_API_ENGINE_IAPI_TBT_FIELD: &str = "engine_iapi_tbt_across_engine_calls_ms";
const RESPONSES_API_ENGINE_SERVICE_TBT_FIELD: &str = "engine_service_tbt_across_engine_calls_ms";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthEnvTelemetryMetadata {
    pub openai_api_key_env_present: bool,
    pub codex_api_key_env_present: bool,
    pub codex_api_key_env_enabled: bool,
    pub provider_env_key_name: Option<String>,
    pub provider_env_key_present: Option<bool>,
    pub refresh_token_url_override_present: bool,
}

#[derive(Debug, Clone)]
pub struct SessionTelemetryMetadata {
    pub(crate) conversation_id: ThreadId,
    pub(crate) auth_mode: Option<String>,
    pub(crate) auth_env: AuthEnvTelemetryMetadata,
    pub(crate) account_id: Option<String>,
    pub(crate) account_email: Option<String>,
    pub(crate) originator: String,
    pub(crate) service_name: Option<String>,
    pub(crate) session_source: String,
    pub(crate) model: String,
    pub(crate) slug: String,
    pub(crate) log_user_prompts: bool,
    pub(crate) app_version: &'static str,
    pub(crate) terminal_type: String,
}

#[derive(Debug, Clone)]
pub struct SessionTelemetry {
    pub(crate) metadata: SessionTelemetryMetadata,
    pub(crate) metrics: Option<MetricsClient>,
    pub(crate) metrics_use_metadata_tags: bool,
}

impl SessionTelemetry {
    pub fn with_auth_env(mut self, auth_env: AuthEnvTelemetryMetadata) -> Self {
        self.metadata.auth_env = auth_env;
        self
    }

    pub fn with_model(mut self, model: &str, slug: &str) -> Self {
        self.metadata.model = model.to_owned();
        self.metadata.slug = slug.to_owned();
        self
    }

    pub fn with_metrics_service_name(mut self, service_name: &str) -> Self {
        self.metadata.service_name = Some(sanitize_metric_tag_value(service_name));
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

    pub fn snapshot_metrics(&self) -> MetricsResult<ResourceMetrics> {
        let Some(metrics) = &self.metrics else {
            return Err(MetricsError::ExporterDisabled);
        };
        metrics.snapshot()
    }

    /// Collect and discard a runtime metrics snapshot to reset delta accumulators.
    pub fn reset_runtime_metrics(&self) {
        if self.metrics.is_none() {
            return;
        }
        if let Err(err) = self.snapshot_metrics() {
            tracing::debug!("runtime metrics reset skipped: {err}");
        }
    }

    /// Collect a runtime metrics summary if debug snapshots are available.
    pub fn runtime_metrics_summary(&self) -> Option<RuntimeMetricsSummary> {
        let snapshot = match self.snapshot_metrics() {
            Ok(snapshot) => snapshot,
            Err(_) => {
                return None;
            }
        };
        let summary = RuntimeMetricsSummary::from_snapshot(&snapshot);
        if summary.is_empty() {
            None
        } else {
            Some(summary)
        }
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
        SessionMetricTagValues {
            auth_mode: self.metadata.auth_mode.as_deref(),
            session_source: self.metadata.session_source.as_str(),
            originator: self.metadata.originator.as_str(),
            service_name: self.metadata.service_name.as_deref(),
            model: self.metadata.model.as_str(),
            app_version: self.metadata.app_version,
        }
        .into_tags()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        conversation_id: ThreadId,
        model: &str,
        slug: &str,
        account_id: Option<String>,
        account_email: Option<String>,
        auth_mode: Option<TelemetryAuthMode>,
        originator: String,
        log_user_prompts: bool,
        terminal_type: String,
        session_source: SessionSource,
    ) -> SessionTelemetry {
        Self {
            metadata: SessionTelemetryMetadata {
                conversation_id,
                auth_mode: auth_mode.map(|m| m.to_string()),
                auth_env: AuthEnvTelemetryMetadata::default(),
                account_id,
                account_email,
                originator: sanitize_metric_tag_value(originator.as_str()),
                service_name: None,
                session_source: session_source.to_string(),
                model: model.to_owned(),
                slug: slug.to_owned(),
                log_user_prompts,
                app_version: env!("CARGO_PKG_VERSION"),
                terminal_type,
            },
            metrics: crate::metrics::global(),
            metrics_use_metadata_tags: true,
        }
    }

    pub fn record_responses(&self, handle_responses_span: &Span, event: &ResponseEvent) {
        handle_responses_span.record("otel.name", SessionTelemetry::responses_type(event));

        match event {
            ResponseEvent::OutputItemDone(item) => {
                handle_responses_span.record("from", "output_item_done");
                if let ResponseItem::FunctionCall { name, .. } = item {
                    handle_responses_span.record("tool_name", name.as_str());
                }
            }
            ResponseEvent::OutputItemAdded(item) => {
                handle_responses_span.record("from", "output_item_added");
                if let ResponseItem::FunctionCall { name, .. } = item {
                    handle_responses_span.record("tool_name", name.as_str());
                }
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn conversation_starts(
        &self,
        provider_name: &str,
        reasoning_effort: Option<ReasoningEffort>,
        reasoning_summary: ReasoningSummary,
        context_window: Option<i64>,
        auto_compact_token_limit: Option<i64>,
        approval_policy: AskForApproval,
        sandbox_policy: SandboxPolicy,
        mcp_servers: Vec<&str>,
        active_profile: Option<String>,
    ) {
        if active_profile.is_some() {
            self.counter(PROFILE_USAGE_METRIC, /*inc*/ 1, &[]);
        }
        log_and_trace_event!(
            self,
            common: {
                event.name = "codex.conversation_starts",
                provider_name = %provider_name,
                auth.env_openai_api_key_present = self.metadata.auth_env.openai_api_key_env_present,
                auth.env_codex_api_key_present = self.metadata.auth_env.codex_api_key_env_present,
                auth.env_codex_api_key_enabled = self.metadata.auth_env.codex_api_key_env_enabled,
                auth.env_provider_key_name = self.metadata.auth_env.provider_env_key_name.as_deref(),
                auth.env_provider_key_present = self.metadata.auth_env.provider_env_key_present,
                auth.env_refresh_token_url_override_present = self.metadata.auth_env.refresh_token_url_override_present,
                reasoning_effort = reasoning_effort.map(|e| e.to_string()),
                reasoning_summary = %reasoning_summary,
                context_window = context_window,
                auto_compact_token_limit = auto_compact_token_limit,
                approval_policy = %approval_policy,
                sandbox_policy = %sandbox_policy,
            },
            log: {
                mcp_servers = mcp_servers.join(", "),
                active_profile = active_profile,
            },
            trace: {
                mcp_server_count = mcp_servers.len() as i64,
                active_profile_present = active_profile.is_some(),
            },
        );
    }

    pub async fn log_request<F, Fut>(&self, attempt: u64, f: F) -> Result<Response, Error>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Response, Error>>,
    {
        let start = Instant::now();
        let response = f().await;
        let duration = start.elapsed();

        let (status, error) = match &response {
            Ok(response) => (Some(response.status().as_u16()), None),
            Err(error) => (error.status().map(|s| s.as_u16()), Some(error.to_string())),
        };
        self.record_api_request(
            attempt,
            status,
            error.as_deref(),
            duration,
            /*auth_header_attached*/ false,
            /*auth_header_name*/ None,
            /*retry_after_unauthorized*/ false,
            /*recovery_mode*/ None,
            /*recovery_phase*/ None,
            "unknown",
            /*request_id*/ None,
            /*cf_ray*/ None,
            /*auth_error*/ None,
            /*auth_error_code*/ None,
        );

        response
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_api_request(
        &self,
        attempt: u64,
        status: Option<u16>,
        error: Option<&str>,
        duration: Duration,
        auth_header_attached: bool,
        auth_header_name: Option<&str>,
        retry_after_unauthorized: bool,
        recovery_mode: Option<&str>,
        recovery_phase: Option<&str>,
        endpoint: &str,
        request_id: Option<&str>,
        cf_ray: Option<&str>,
        auth_error: Option<&str>,
        auth_error_code: Option<&str>,
    ) {
        let success = status.is_some_and(|code| (200..=299).contains(&code)) && error.is_none();
        let success_str = if success { "true" } else { "false" };
        let status_str = status
            .map(|code| code.to_string())
            .unwrap_or_else(|| "none".to_string());
        self.counter(
            API_CALL_COUNT_METRIC,
            /*inc*/ 1,
            &[("status", status_str.as_str()), ("success", success_str)],
        );
        self.record_duration(
            API_CALL_DURATION_METRIC,
            duration,
            &[("status", status_str.as_str()), ("success", success_str)],
        );
        log_and_trace_event!(
            self,
            common: {
                event.name = "codex.api_request",
                duration_ms = %duration.as_millis(),
                http.response.status_code = status,
                error.message = error,
                attempt = attempt,
                auth.header_attached = auth_header_attached,
                auth.header_name = auth_header_name,
                auth.retry_after_unauthorized = retry_after_unauthorized,
                auth.recovery_mode = recovery_mode,
                auth.recovery_phase = recovery_phase,
                endpoint = endpoint,
                auth.env_openai_api_key_present = self.metadata.auth_env.openai_api_key_env_present,
                auth.env_codex_api_key_present = self.metadata.auth_env.codex_api_key_env_present,
                auth.env_codex_api_key_enabled = self.metadata.auth_env.codex_api_key_env_enabled,
                auth.env_provider_key_name = self.metadata.auth_env.provider_env_key_name.as_deref(),
                auth.env_provider_key_present = self.metadata.auth_env.provider_env_key_present,
                auth.env_refresh_token_url_override_present = self.metadata.auth_env.refresh_token_url_override_present,
                auth.request_id = request_id,
                auth.cf_ray = cf_ray,
                auth.error = auth_error,
                auth.error_code = auth_error_code,
            },
            log: {},
            trace: {},
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_websocket_connect(
        &self,
        duration: Duration,
        status: Option<u16>,
        error: Option<&str>,
        auth_header_attached: bool,
        auth_header_name: Option<&str>,
        retry_after_unauthorized: bool,
        recovery_mode: Option<&str>,
        recovery_phase: Option<&str>,
        endpoint: &str,
        connection_reused: bool,
        request_id: Option<&str>,
        cf_ray: Option<&str>,
        auth_error: Option<&str>,
        auth_error_code: Option<&str>,
    ) {
        let success = error.is_none()
            && status
                .map(|code| (200..=299).contains(&code))
                .unwrap_or(true);
        let success_str = if success { "true" } else { "false" };
        log_and_trace_event!(
            self,
            common: {
                event.name = "codex.websocket_connect",
                duration_ms = %duration.as_millis(),
                http.response.status_code = status,
                success = success_str,
                error.message = error,
                auth.header_attached = auth_header_attached,
                auth.header_name = auth_header_name,
                auth.retry_after_unauthorized = retry_after_unauthorized,
                auth.recovery_mode = recovery_mode,
                auth.recovery_phase = recovery_phase,
                endpoint = endpoint,
                auth.env_openai_api_key_present = self.metadata.auth_env.openai_api_key_env_present,
                auth.env_codex_api_key_present = self.metadata.auth_env.codex_api_key_env_present,
                auth.env_codex_api_key_enabled = self.metadata.auth_env.codex_api_key_env_enabled,
                auth.env_provider_key_name = self.metadata.auth_env.provider_env_key_name.as_deref(),
                auth.env_provider_key_present = self.metadata.auth_env.provider_env_key_present,
                auth.env_refresh_token_url_override_present = self.metadata.auth_env.refresh_token_url_override_present,
                auth.connection_reused = connection_reused,
                auth.request_id = request_id,
                auth.cf_ray = cf_ray,
                auth.error = auth_error,
                auth.error_code = auth_error_code,
            },
            log: {},
            trace: {},
        );
    }

    pub fn record_websocket_request(
        &self,
        duration: Duration,
        error: Option<&str>,
        connection_reused: bool,
    ) {
        let success_str = if error.is_none() { "true" } else { "false" };
        self.counter(
            WEBSOCKET_REQUEST_COUNT_METRIC,
            /*inc*/ 1,
            &[("success", success_str)],
        );
        self.record_duration(
            WEBSOCKET_REQUEST_DURATION_METRIC,
            duration,
            &[("success", success_str)],
        );
        log_and_trace_event!(
            self,
            common: {
                event.name = "codex.websocket_request",
                duration_ms = %duration.as_millis(),
                success = success_str,
                error.message = error,
                auth.env_openai_api_key_present = self.metadata.auth_env.openai_api_key_env_present,
                auth.env_codex_api_key_present = self.metadata.auth_env.codex_api_key_env_present,
                auth.env_codex_api_key_enabled = self.metadata.auth_env.codex_api_key_env_enabled,
                auth.env_provider_key_name = self.metadata.auth_env.provider_env_key_name.as_deref(),
                auth.env_provider_key_present = self.metadata.auth_env.provider_env_key_present,
                auth.env_refresh_token_url_override_present = self.metadata.auth_env.refresh_token_url_override_present,
                auth.connection_reused = connection_reused,
            },
            log: {},
            trace: {},
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_auth_recovery(
        &self,
        mode: &str,
        step: &str,
        outcome: &str,
        request_id: Option<&str>,
        cf_ray: Option<&str>,
        auth_error: Option<&str>,
        auth_error_code: Option<&str>,
        recovery_reason: Option<&str>,
        auth_state_changed: Option<bool>,
    ) {
        log_and_trace_event!(
            self,
            common: {
                event.name = "codex.auth_recovery",
                auth.mode = mode,
                auth.step = step,
                auth.outcome = outcome,
                auth.request_id = request_id,
                auth.cf_ray = cf_ray,
                auth.error = auth_error,
                auth.error_code = auth_error_code,
                auth.recovery_reason = recovery_reason,
                auth.state_changed = auth_state_changed,
            },
            log: {},
            trace: {},
        );
    }

    pub fn record_websocket_event(
        &self,
        result: &Result<
            Option<
                Result<
                    tokio_tungstenite::tungstenite::Message,
                    tokio_tungstenite::tungstenite::Error,
                >,
            >,
            ApiError,
        >,
        duration: Duration,
    ) {
        let mut kind = None;
        let mut error_message = None;
        let mut success = true;

        match result {
            Ok(Some(Ok(message))) => match message {
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    match serde_json::from_str::<serde_json::Value>(text) {
                        Ok(value) => {
                            kind = value
                                .get("type")
                                .and_then(|value| value.as_str())
                                .map(std::string::ToString::to_string);
                            if kind.as_deref() == Some(RESPONSES_WEBSOCKET_TIMING_KIND) {
                                self.record_responses_websocket_timing_metrics(&value);
                            }
                            if kind.as_deref() == Some("response.failed") {
                                success = false;
                                error_message = value
                                    .get("response")
                                    .and_then(|value| value.get("error"))
                                    .map(serde_json::Value::to_string)
                                    .or_else(|| Some("response.failed event received".to_string()));
                            }
                        }
                        Err(err) => {
                            kind = Some("parse_error".to_string());
                            error_message = Some(err.to_string());
                            success = false;
                        }
                    }
                }
                tokio_tungstenite::tungstenite::Message::Binary(_) => {
                    success = false;
                    error_message = Some("unexpected binary websocket event".to_string());
                }
                tokio_tungstenite::tungstenite::Message::Ping(_)
                | tokio_tungstenite::tungstenite::Message::Pong(_) => {
                    return;
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => {
                    success = false;
                    error_message =
                        Some("websocket closed by server before response.completed".to_string());
                }
                tokio_tungstenite::tungstenite::Message::Frame(_) => {
                    success = false;
                    error_message = Some("unexpected websocket frame".to_string());
                }
            },
            Ok(Some(Err(err))) => {
                success = false;
                error_message = Some(err.to_string());
            }
            Ok(None) => {
                success = false;
                error_message = Some("stream closed before response.completed".to_string());
            }
            Err(err) => {
                success = false;
                error_message = Some(err.to_string());
            }
        }

        let kind_str = kind.as_deref().unwrap_or(WEBSOCKET_UNKNOWN_KIND);
        let success_str = if success { "true" } else { "false" };
        let tags = [("kind", kind_str), ("success", success_str)];
        self.counter(WEBSOCKET_EVENT_COUNT_METRIC, /*inc*/ 1, &tags);
        self.record_duration(WEBSOCKET_EVENT_DURATION_METRIC, duration, &tags);
        log_and_trace_event!(
            self,
            common: {
                event.name = "codex.websocket_event",
                event.kind = %kind_str,
                duration_ms = %duration.as_millis(),
                success = success_str,
                error.message = error_message.as_deref(),
            },
            log: {},
            trace: {},
        );
    }

    pub fn log_sse_event<E>(
        &self,
        response: &Result<Option<Result<StreamEvent, StreamError<E>>>, Elapsed>,
        duration: Duration,
    ) where
        E: std::fmt::Display,
    {
        match response {
            Ok(Some(Ok(sse))) => {
                if sse.data.trim() == "[DONE]" {
                    self.sse_event(&sse.event, duration);
                } else {
                    match serde_json::from_str::<serde_json::Value>(&sse.data) {
                        Ok(error) if sse.event == "response.failed" => {
                            self.sse_event_failed(Some(&sse.event), duration, &error);
                        }
                        Ok(content) if sse.event == "response.output_item.done" => {
                            match serde_json::from_value::<ResponseItem>(content) {
                                Ok(_) => self.sse_event(&sse.event, duration),
                                Err(_) => {
                                    self.sse_event_failed(
                                        Some(&sse.event),
                                        duration,
                                        &"failed to parse response.output_item.done",
                                    );
                                }
                            };
                        }
                        Ok(_) => {
                            self.sse_event(&sse.event, duration);
                        }
                        Err(error) => {
                            self.sse_event_failed(Some(&sse.event), duration, &error);
                        }
                    }
                }
            }
            Ok(Some(Err(error))) => {
                self.sse_event_failed(/*kind*/ None, duration, error);
            }
            Ok(None) => {}
            Err(_) => {
                self.sse_event_failed(
                    /*kind*/ None,
                    duration,
                    &"idle timeout waiting for SSE",
                );
            }
        }
    }

    fn sse_event(&self, kind: &str, duration: Duration) {
        self.counter(
            SSE_EVENT_COUNT_METRIC,
            /*inc*/ 1,
            &[("kind", kind), ("success", "true")],
        );
        self.record_duration(
            SSE_EVENT_DURATION_METRIC,
            duration,
            &[("kind", kind), ("success", "true")],
        );
        log_event!(
            self,
            event.name = "codex.sse_event",
            event.kind = %kind,
            duration_ms = %duration.as_millis(),
        );
    }

    pub fn sse_event_failed<T>(&self, kind: Option<&String>, duration: Duration, error: &T)
    where
        T: std::fmt::Display,
    {
        let kind_str = kind.map_or(SSE_UNKNOWN_KIND, String::as_str);
        self.counter(
            SSE_EVENT_COUNT_METRIC,
            /*inc*/ 1,
            &[("kind", kind_str), ("success", "false")],
        );
        self.record_duration(
            SSE_EVENT_DURATION_METRIC,
            duration,
            &[("kind", kind_str), ("success", "false")],
        );
        match kind {
            Some(kind) => log_event!(
                self,
                event.name = "codex.sse_event",
                event.kind = %kind,
                duration_ms = %duration.as_millis(),
                error.message = %error,
            ),
            None => log_event!(
                self,
                event.name = "codex.sse_event",
                duration_ms = %duration.as_millis(),
                error.message = %error,
            ),
        }
        trace_event!(
            self,
            event.name = "codex.sse_event",
            event.kind = %kind_str,
            duration_ms = %duration.as_millis(),
            error.message = %error,
        );
    }

    pub fn see_event_completed_failed<T>(&self, error: &T)
    where
        T: std::fmt::Display,
    {
        log_and_trace_event!(
            self,
            common: {
                event.name = "codex.sse_event",
                event.kind = %"response.completed",
                error.message = %error,
            },
            log: {},
            trace: {},
        );
    }

    pub fn sse_event_completed(
        &self,
        input_token_count: i64,
        output_token_count: i64,
        cached_token_count: Option<i64>,
        reasoning_token_count: Option<i64>,
        tool_token_count: i64,
    ) {
        log_and_trace_event!(
            self,
            common: {
                event.name = "codex.sse_event",
                event.kind = %"response.completed",
                input_token_count = %input_token_count,
                output_token_count = %output_token_count,
                cached_token_count = cached_token_count,
                reasoning_token_count = reasoning_token_count,
                tool_token_count = %tool_token_count,
            },
            log: {},
            trace: {},
        );
    }

    pub fn user_prompt(&self, items: &[UserInput]) {
        let prompt = items
            .iter()
            .flat_map(|item| match item {
                UserInput::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();
        let text_input_count = items
            .iter()
            .filter(|item| matches!(item, UserInput::Text { .. }))
            .count();
        let image_input_count = items
            .iter()
            .filter(|item| matches!(item, UserInput::Image { .. }))
            .count();
        let local_image_input_count = items
            .iter()
            .filter(|item| matches!(item, UserInput::LocalImage { .. }))
            .count();

        let prompt_to_log = if self.metadata.log_user_prompts {
            prompt.as_str()
        } else {
            "[REDACTED]"
        };

        log_event!(
            self,
            event.name = "codex.user_prompt",
            prompt_length = %prompt.chars().count(),
            prompt = %prompt_to_log,
        );
        trace_event!(
            self,
            event.name = "codex.user_prompt",
            prompt_length = %prompt.chars().count(),
            text_input_count = text_input_count as i64,
            image_input_count = image_input_count as i64,
            local_image_input_count = local_image_input_count as i64,
        );
    }

    pub fn tool_decision(
        &self,
        tool_name: &str,
        call_id: &str,
        decision: &ReviewDecision,
        source: ToolDecisionSource,
        execpolicy_matched: Option<bool>,
    ) {
        log_event!(
            self,
            event.name = "codex.tool_decision",
            tool_name = %tool_name,
            call_id = %call_id,
            decision = %decision.clone().to_string().to_lowercase(),
            source = %source.to_string(),
            execpolicy_matched = %execpolicy_matched
                .map(|matched| matched.to_string())
                .unwrap_or_default(),
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn log_tool_result_with_tags<F, Fut, E>(
        &self,
        tool_name: &str,
        call_id: &str,
        arguments: &str,
        extra_tags: &[(&str, &str)],
        mcp_server: Option<&str>,
        mcp_server_origin: Option<&str>,
        f: F,
    ) -> Result<(String, bool), E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(String, bool), E>>,
        E: std::fmt::Display,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed();

        let (output, success) = match &result {
            Ok((preview, success)) => (Cow::Borrowed(preview.as_str()), *success),
            Err(error) => (Cow::Owned(error.to_string()), false),
        };

        self.tool_result_with_tags(
            tool_name,
            call_id,
            arguments,
            duration,
            success,
            output.as_ref(),
            extra_tags,
            mcp_server,
            mcp_server_origin,
        );

        result
    }

    pub fn log_tool_failed(&self, tool_name: &str, error: &str) {
        log_event!(
            self,
            event.name = "codex.tool_result",
            tool_name = %tool_name,
            duration_ms = %Duration::ZERO.as_millis(),
            success = %false,
            output = %error,
            mcp_server = "",
            mcp_server_origin = "",
        );
        trace_event!(
            self,
            event.name = "codex.tool_result",
            tool_name = %tool_name,
            duration_ms = %Duration::ZERO.as_millis(),
            success = %false,
            output_length = error.len() as i64,
            output_line_count = error.lines().count() as i64,
            tool_origin = %"builtin",
            error.message = %error,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn tool_result_with_tags(
        &self,
        tool_name: &str,
        call_id: &str,
        arguments: &str,
        duration: Duration,
        success: bool,
        output: &str,
        extra_tags: &[(&str, &str)],
        mcp_server: Option<&str>,
        mcp_server_origin: Option<&str>,
    ) {
        let success_str = if success { "true" } else { "false" };
        let mut tags = Vec::with_capacity(2 + extra_tags.len());
        tags.push(("tool", tool_name));
        tags.push(("success", success_str));
        tags.extend_from_slice(extra_tags);
        self.counter(TOOL_CALL_COUNT_METRIC, /*inc*/ 1, &tags);
        self.record_duration(TOOL_CALL_DURATION_METRIC, duration, &tags);
        let mcp_server = mcp_server.unwrap_or("");
        let mcp_server_origin = mcp_server_origin.unwrap_or("");
        log_event!(
            self,
            event.name = "codex.tool_result",
            tool_name = %tool_name,
            call_id = %call_id,
            arguments = %arguments,
            duration_ms = %duration.as_millis(),
            success = %success_str,
            output = %output,
            mcp_server = %mcp_server,
            mcp_server_origin = %mcp_server_origin,
        );
        trace_event!(
            self,
            event.name = "codex.tool_result",
            tool_name = %tool_name,
            call_id = %call_id,
            duration_ms = %duration.as_millis(),
            success = %success_str,
            arguments_length = arguments.len() as i64,
            output_length = output.len() as i64,
            output_line_count = output.lines().count() as i64,
            tool_origin = if mcp_server.is_empty() { "builtin" } else { "mcp" },
            mcp_tool = !mcp_server.is_empty(),
        );
    }

    fn record_responses_websocket_timing_metrics(&self, value: &serde_json::Value) {
        let timing_metrics = value.get(RESPONSES_WEBSOCKET_TIMING_METRICS_FIELD);

        let overhead_value =
            timing_metrics.and_then(|value| value.get(RESPONSES_API_OVERHEAD_FIELD));
        if let Some(duration) = duration_from_ms_value(overhead_value) {
            self.record_duration(RESPONSES_API_OVERHEAD_DURATION_METRIC, duration, &[]);
        }

        let inference_value =
            timing_metrics.and_then(|value| value.get(RESPONSES_API_INFERENCE_FIELD));
        if let Some(duration) = duration_from_ms_value(inference_value) {
            self.record_duration(RESPONSES_API_INFERENCE_TIME_DURATION_METRIC, duration, &[]);
        }

        let engine_iapi_ttft_value =
            timing_metrics.and_then(|value| value.get(RESPONSES_API_ENGINE_IAPI_TTFT_FIELD));
        if let Some(duration) = duration_from_ms_value(engine_iapi_ttft_value) {
            self.record_duration(
                RESPONSES_API_ENGINE_IAPI_TTFT_DURATION_METRIC,
                duration,
                &[],
            );
        }

        let engine_service_ttft_value =
            timing_metrics.and_then(|value| value.get(RESPONSES_API_ENGINE_SERVICE_TTFT_FIELD));
        if let Some(duration) = duration_from_ms_value(engine_service_ttft_value) {
            self.record_duration(
                RESPONSES_API_ENGINE_SERVICE_TTFT_DURATION_METRIC,
                duration,
                &[],
            );
        }

        let engine_iapi_tbt_value =
            timing_metrics.and_then(|value| value.get(RESPONSES_API_ENGINE_IAPI_TBT_FIELD));
        if let Some(duration) = duration_from_ms_value(engine_iapi_tbt_value) {
            self.record_duration(RESPONSES_API_ENGINE_IAPI_TBT_DURATION_METRIC, duration, &[]);
        }

        let engine_service_tbt_value =
            timing_metrics.and_then(|value| value.get(RESPONSES_API_ENGINE_SERVICE_TBT_FIELD));
        if let Some(duration) = duration_from_ms_value(engine_service_tbt_value) {
            self.record_duration(
                RESPONSES_API_ENGINE_SERVICE_TBT_DURATION_METRIC,
                duration,
                &[],
            );
        }
    }

    fn responses_type(event: &ResponseEvent) -> String {
        match event {
            ResponseEvent::Created => "created".into(),
            ResponseEvent::OutputItemDone(item) | ResponseEvent::OutputItemAdded(item) => {
                SessionTelemetry::responses_item_type(item)
            }
            ResponseEvent::Completed { .. } => "completed".into(),
            ResponseEvent::OutputTextDelta(_) => "text_delta".into(),
            ResponseEvent::ToolCallInputDelta { .. } => "tool_input_delta".into(),
            ResponseEvent::ReasoningSummaryDelta { .. } => "reasoning_summary_delta".into(),
            ResponseEvent::ReasoningContentDelta { .. } => "reasoning_content_delta".into(),
            ResponseEvent::ReasoningSummaryPartAdded { .. } => {
                "reasoning_summary_part_added".into()
            }
            ResponseEvent::ServerModel(_) => "server_model".into(),
            ResponseEvent::ServerReasoningIncluded(_) => "server_reasoning_included".into(),
            ResponseEvent::RateLimits(_) => "rate_limits".into(),
            ResponseEvent::ModelsEtag(_) => "models_etag".into(),
        }
    }

    fn responses_item_type(item: &ResponseItem) -> String {
        match item {
            ResponseItem::Message { role, .. } => format!("message_from_{role}"),
            ResponseItem::Reasoning { .. } => "reasoning".into(),
            ResponseItem::LocalShellCall { .. } => "local_shell_call".into(),
            ResponseItem::FunctionCall { .. } => "function_call".into(),
            ResponseItem::ToolSearchCall { .. } => "tool_search_call".into(),
            ResponseItem::FunctionCallOutput { .. } => "function_call_output".into(),
            ResponseItem::ToolSearchOutput { .. } => "tool_search_output".into(),
            ResponseItem::CustomToolCall { .. } => "custom_tool_call".into(),
            ResponseItem::CustomToolCallOutput { .. } => "custom_tool_call_output".into(),
            ResponseItem::WebSearchCall { .. } => "web_search_call".into(),
            ResponseItem::ImageGenerationCall { .. } => "image_generation_call".into(),
            ResponseItem::GhostSnapshot { .. } => "ghost_snapshot".into(),
            ResponseItem::Compaction { .. } => "compaction".into(),
            ResponseItem::Other => "other".into(),
        }
    }
}

fn duration_from_ms_value(value: Option<&serde_json::Value>) -> Option<Duration> {
    let value = value?;
    let ms = value
        .as_f64()
        .or_else(|| value.as_i64().map(|v| v as f64))
        .or_else(|| value.as_u64().map(|v| v as f64))?;
    if !ms.is_finite() || ms < 0.0 {
        return None;
    }
    let clamped = ms.min(u64::MAX as f64);
    Some(Duration::from_millis(clamped.round() as u64))
}
