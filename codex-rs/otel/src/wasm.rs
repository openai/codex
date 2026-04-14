use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::W3cTraceContext;
use codex_protocol::user_input::UserInput;
use serde::Serialize;
use std::future::Future;
use std::marker::PhantomData;
use std::time::Duration;
use tracing::Span;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context as LayerContext;
use tracing_subscriber::registry::LookupSpan;

pub mod config {
    use serde::Deserialize;
    use serde::Serialize;

    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    pub struct OtelSettings {
        pub environment: String,
        pub service_name: String,
        pub service_version: String,
        pub exporter: OtelExporter,
        pub trace_exporter: OtelExporter,
        pub metrics_exporter: OtelExporter,
        pub http_protocol: OtelHttpProtocol,
        pub tls: Option<OtelTlsConfig>,
        pub runtime_metrics: bool,
    }

    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum OtelHttpProtocol {
        #[default]
        HttpProtobuf,
        HttpJson,
    }

    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    pub struct OtelTlsConfig {
        pub ca_cert_path: Option<String>,
    }

    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum OtelExporter {
        #[default]
        None,
        Otlp,
    }
}

pub mod metrics {
    use std::sync::OnceLock;
    use std::time::Duration;

    pub mod names {
        pub const TOOL_CALL_COUNT_METRIC: &str = "codex.tool.call";
        pub const TOOL_CALL_DURATION_METRIC: &str = "codex.tool.call.duration_ms";
        pub const TOOL_CALL_UNIFIED_EXEC_METRIC: &str = "codex.tool.unified_exec";
        pub const API_CALL_COUNT_METRIC: &str = "codex.api_request";
        pub const API_CALL_DURATION_METRIC: &str = "codex.api_request.duration_ms";
        pub const SSE_EVENT_COUNT_METRIC: &str = "codex.sse_event";
        pub const SSE_EVENT_DURATION_METRIC: &str = "codex.sse_event.duration_ms";
        pub const WEBSOCKET_REQUEST_COUNT_METRIC: &str = "codex.websocket.request";
        pub const WEBSOCKET_REQUEST_DURATION_METRIC: &str = "codex.websocket.request.duration_ms";
        pub const WEBSOCKET_EVENT_COUNT_METRIC: &str = "codex.websocket.event";
        pub const WEBSOCKET_EVENT_DURATION_METRIC: &str = "codex.websocket.event.duration_ms";
        pub const RESPONSES_API_OVERHEAD_DURATION_METRIC: &str =
            "codex.responses_api_overhead.duration_ms";
        pub const RESPONSES_API_INFERENCE_TIME_DURATION_METRIC: &str =
            "codex.responses_api_inference_time.duration_ms";
        pub const RESPONSES_API_ENGINE_IAPI_TTFT_DURATION_METRIC: &str =
            "codex.responses_api_engine_iapi_ttft.duration_ms";
        pub const RESPONSES_API_ENGINE_SERVICE_TTFT_DURATION_METRIC: &str =
            "codex.responses_api_engine_service_ttft.duration_ms";
        pub const RESPONSES_API_ENGINE_IAPI_TBT_DURATION_METRIC: &str =
            "codex.responses_api_engine_iapi_tbt.duration_ms";
        pub const RESPONSES_API_ENGINE_SERVICE_TBT_DURATION_METRIC: &str =
            "codex.responses_api_engine_service_tbt.duration_ms";
        pub const TURN_E2E_DURATION_METRIC: &str = "codex.turn.e2e_duration_ms";
        pub const TURN_TTFT_DURATION_METRIC: &str = "codex.turn.ttft.duration_ms";
        pub const TURN_TTFM_DURATION_METRIC: &str = "codex.turn.ttfm.duration_ms";
        pub const TURN_NETWORK_PROXY_METRIC: &str = "codex.turn.network_proxy";
        pub const TURN_TOOL_CALL_METRIC: &str = "codex.turn.tool.call";
        pub const TURN_TOKEN_USAGE_METRIC: &str = "codex.turn.token_usage";
        pub const PROFILE_USAGE_METRIC: &str = "codex.profile.usage";
        pub const CURATED_PLUGINS_STARTUP_SYNC_METRIC: &str = "codex.plugins.startup_sync";
        pub const CURATED_PLUGINS_STARTUP_SYNC_FINAL_METRIC: &str =
            "codex.plugins.startup_sync.final";
        pub const STARTUP_PREWARM_DURATION_METRIC: &str = "codex.startup_prewarm.duration_ms";
        pub const STARTUP_PREWARM_AGE_AT_FIRST_TURN_METRIC: &str =
            "codex.startup_prewarm.age_at_first_turn_ms";
        pub const THREAD_STARTED_METRIC: &str = "codex.thread.started";
    }

    pub mod runtime_metrics {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct RuntimeMetricTotals {
            pub count: i64,
            pub duration_ms: i64,
        }

        impl RuntimeMetricTotals {
            pub fn is_empty(self) -> bool {
                self.count == 0 && self.duration_ms == 0
            }

            pub fn merge(&mut self, other: Self) {
                self.count += other.count;
                self.duration_ms += other.duration_ms;
            }
        }

        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct RuntimeMetricsSummary {
            pub tool_calls: RuntimeMetricTotals,
            pub api_calls: RuntimeMetricTotals,
            pub streaming_events: RuntimeMetricTotals,
            pub websocket_calls: RuntimeMetricTotals,
            pub websocket_events: RuntimeMetricTotals,
        }

        impl RuntimeMetricsSummary {
            pub fn is_empty(self) -> bool {
                self.tool_calls.is_empty()
                    && self.api_calls.is_empty()
                    && self.streaming_events.is_empty()
                    && self.websocket_calls.is_empty()
                    && self.websocket_events.is_empty()
            }

            pub fn merge(&mut self, other: Self) {
                self.tool_calls.merge(other.tool_calls);
                self.api_calls.merge(other.api_calls);
                self.streaming_events.merge(other.streaming_events);
                self.websocket_calls.merge(other.websocket_calls);
                self.websocket_events.merge(other.websocket_events);
            }

            pub fn responses_api_summary(&self) -> RuntimeMetricsSummary {
                *self
            }
        }
    }

    pub mod error {
        pub type Result<T> = std::result::Result<T, MetricsError>;

        #[derive(Debug, Clone, thiserror::Error)]
        pub enum MetricsError {
            #[error("metrics exporter disabled")]
            ExporterDisabled,
        }
    }

    pub mod config {
        use std::time::Duration;

        use crate::config::OtelExporter;

        #[derive(Clone, Debug, Default)]
        pub enum MetricsExporter {
            #[default]
            None,
            Otlp,
            InMemory,
        }

        #[derive(Clone, Debug, Default)]
        pub struct MetricsConfig {
            pub environment: String,
            pub service_name: String,
            pub service_version: String,
            pub exporter: MetricsExporter,
            pub export_interval: Option<Duration>,
            pub runtime_reader: bool,
            pub tags: Vec<(String, String)>,
        }

        impl MetricsConfig {
            pub fn otlp(
                environment: String,
                service_name: String,
                service_version: String,
                exporter: OtelExporter,
            ) -> Self {
                Self {
                    environment,
                    service_name,
                    service_version,
                    exporter: match exporter {
                        OtelExporter::None => MetricsExporter::None,
                        OtelExporter::Otlp => MetricsExporter::Otlp,
                    },
                    export_interval: None,
                    runtime_reader: false,
                    tags: Vec::new(),
                }
            }

            pub fn in_memory<T>(
                environment: impl Into<String>,
                service_name: impl Into<String>,
                service_version: impl Into<String>,
                _exporter: T,
            ) -> Self {
                Self {
                    environment: environment.into(),
                    service_name: service_name.into(),
                    service_version: service_version.into(),
                    exporter: MetricsExporter::InMemory,
                    export_interval: None,
                    runtime_reader: false,
                    tags: Vec::new(),
                }
            }

            pub fn with_export_interval(mut self, interval: Duration) -> Self {
                self.export_interval = Some(interval);
                self
            }

            pub fn with_runtime_reader(mut self) -> Self {
                self.runtime_reader = true;
                self
            }

            pub fn with_tag(
                mut self,
                key: impl Into<String>,
                value: impl Into<String>,
            ) -> crate::metrics::error::Result<Self> {
                self.tags.push((key.into(), value.into()));
                Ok(self)
            }
        }
    }

    pub mod data {
        #[derive(Clone, Debug, Default)]
        pub struct ResourceMetrics;
    }

    pub mod client {
        use std::time::Duration;

        use crate::metrics::config::MetricsConfig;
        use crate::metrics::data::ResourceMetrics;
        use crate::metrics::error::Result;
        use crate::metrics::runtime_metrics::RuntimeMetricsSummary;
        use crate::metrics::timer::Timer;

        #[derive(Clone, Debug, Default)]
        pub struct MetricsClient;

        impl MetricsClient {
            pub fn new(_config: MetricsConfig) -> Result<Self> {
                Ok(Self)
            }

            pub fn counter(&self, _name: &str, _inc: i64, _tags: &[(&str, &str)]) -> Result<()> {
                Ok(())
            }

            pub fn histogram(
                &self,
                _name: &str,
                _value: i64,
                _tags: &[(&str, &str)],
            ) -> Result<()> {
                Ok(())
            }

            pub fn record_duration(
                &self,
                _name: &str,
                _duration: Duration,
                _tags: &[(&str, &str)],
            ) -> Result<()> {
                Ok(())
            }

            pub fn start_timer(&self, name: &str, tags: &[(&str, &str)]) -> Result<Timer> {
                Ok(Timer::new(name, tags, self))
            }

            pub fn snapshot(&self) -> Result<ResourceMetrics> {
                Ok(ResourceMetrics)
            }

            pub fn shutdown(&self) -> Result<()> {
                Ok(())
            }

            pub fn reset_runtime_metrics(&self) {}

            pub fn runtime_metrics_summary(&self) -> Option<RuntimeMetricsSummary> {
                Some(RuntimeMetricsSummary::default())
            }
        }
    }

    pub mod timer {
        use std::time::Duration;
        use std::time::Instant;

        use crate::metrics::client::MetricsClient;
        use crate::metrics::error::Result;

        #[derive(Clone, Debug)]
        pub struct Timer {
            name: String,
            tags: Vec<(String, String)>,
            start: Instant,
            client: MetricsClient,
        }

        impl Timer {
            pub(crate) fn new(name: &str, tags: &[(&str, &str)], client: &MetricsClient) -> Self {
                Self {
                    name: name.to_string(),
                    tags: tags
                        .iter()
                        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                        .collect(),
                    start: Instant::now(),
                    client: client.clone(),
                }
            }

            pub fn record(&self, additional_tags: &[(&str, &str)]) -> Result<()> {
                let mut tags: Vec<(&str, &str)> = self
                    .tags
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                tags.extend_from_slice(additional_tags);
                self.client.record_duration(
                    &self.name,
                    self.start.elapsed().max(Duration::ZERO),
                    &tags,
                )
            }
        }

        impl Drop for Timer {
            fn drop(&mut self) {
                let _ = self.record(&[]);
            }
        }
    }

    pub use client::MetricsClient;
    pub use config::MetricsConfig;
    pub use error::MetricsError;
    pub use error::Result;
    pub use runtime_metrics::RuntimeMetricTotals;
    pub use runtime_metrics::RuntimeMetricsSummary;
    pub use timer::Timer;

    static GLOBAL_METRICS: OnceLock<MetricsClient> = OnceLock::new();

    pub(crate) fn install_global(metrics: MetricsClient) {
        let _ = GLOBAL_METRICS.set(metrics);
    }

    pub fn global() -> Option<MetricsClient> {
        GLOBAL_METRICS.get().cloned()
    }
}

pub mod provider {
    use std::error::Error;

    use tracing_subscriber::Layer;
    use tracing_subscriber::registry::LookupSpan;

    use crate::config::OtelSettings;
    use crate::metrics::MetricsClient;

    #[derive(Clone, Debug, Default)]
    pub struct NoopLayer;

    impl<S> Layer<S> for NoopLayer where S: tracing::Subscriber + for<'span> LookupSpan<'span> {}

    #[derive(Clone, Debug, Default)]
    pub struct OtelProvider {
        pub metrics: Option<MetricsClient>,
    }

    impl OtelProvider {
        pub fn shutdown(&self) {}

        pub fn from(_settings: &OtelSettings) -> Result<Option<Self>, Box<dyn Error>> {
            Ok(None)
        }

        pub fn logger_layer<S>(&self) -> Option<impl Layer<S> + Send + Sync>
        where
            S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
        {
            Some(NoopLayer)
        }

        pub fn tracing_layer<S>(&self) -> Option<impl Layer<S> + Send + Sync>
        where
            S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
        {
            Some(NoopLayer)
        }

        pub fn codex_export_filter(_meta: &tracing::Metadata<'_>) -> bool {
            false
        }

        pub fn log_export_filter(_meta: &tracing::Metadata<'_>) -> bool {
            false
        }

        pub fn trace_export_filter(_meta: &tracing::Metadata<'_>) -> bool {
            false
        }

        pub fn metrics(&self) -> Option<&MetricsClient> {
            self.metrics.as_ref()
        }
    }
}

pub mod trace_context {
    use codex_protocol::protocol::W3cTraceContext;
    use tracing::Span;

    #[derive(Clone, Debug, Default)]
    pub struct Context;

    pub fn current_span_w3c_trace_context() -> Option<W3cTraceContext> {
        None
    }

    pub fn span_w3c_trace_context(_span: &Span) -> Option<W3cTraceContext> {
        None
    }

    pub fn current_span_trace_id() -> Option<String> {
        None
    }

    pub fn context_from_w3c_trace_context(_trace: &W3cTraceContext) -> Option<Context> {
        None
    }

    pub fn set_parent_from_w3c_trace_context(_span: &Span, _trace: &W3cTraceContext) -> bool {
        false
    }

    pub fn set_parent_from_context(_span: &Span, _context: Context) {}

    pub fn traceparent_context_from_env() -> Option<Context> {
        None
    }
}

pub use crate::metrics::runtime_metrics::RuntimeMetricTotals;
pub use crate::metrics::runtime_metrics::RuntimeMetricsSummary;
pub use crate::metrics::timer::Timer;
pub use crate::provider::OtelProvider;
pub use crate::trace_context::Context;
pub use crate::trace_context::context_from_w3c_trace_context;
pub use crate::trace_context::current_span_trace_id;
pub use crate::trace_context::current_span_w3c_trace_context;
pub use crate::trace_context::set_parent_from_context;
pub use crate::trace_context::set_parent_from_w3c_trace_context;
pub use crate::trace_context::span_w3c_trace_context;
pub use crate::trace_context::traceparent_context_from_env;
pub use codex_utils_string::sanitize_metric_tag_value;

use crate::metrics::MetricsClient;
use crate::metrics::MetricsConfig;
use crate::metrics::MetricsError;
use crate::metrics::Result as MetricsResult;
use crate::provider::NoopLayer;

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

#[derive(Debug, Clone, Default)]
pub struct SessionTelemetry {
    pub(crate) metadata: Option<SessionTelemetryMetadata>,
    pub(crate) metrics: Option<MetricsClient>,
    pub(crate) metrics_use_metadata_tags: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolDecisionSource {
    AutomatedReviewer,
    Config,
    User,
}

impl std::fmt::Display for ToolDecisionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AutomatedReviewer => f.write_str("automated_reviewer"),
            Self::Config => f.write_str("config"),
            Self::User => f.write_str("user"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryAuthMode {
    ApiKey,
    Chatgpt,
}

impl std::fmt::Display for TelemetryAuthMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiKey => f.write_str("api_key"),
            Self::Chatgpt => f.write_str("chatgpt"),
        }
    }
}

impl From<codex_app_server_protocol::AuthMode> for TelemetryAuthMode {
    fn from(mode: codex_app_server_protocol::AuthMode) -> Self {
        match mode {
            codex_app_server_protocol::AuthMode::ApiKey => Self::ApiKey,
            codex_app_server_protocol::AuthMode::Chatgpt
            | codex_app_server_protocol::AuthMode::ChatgptAuthTokens => Self::Chatgpt,
        }
    }
}

impl SessionTelemetry {
    pub fn with_auth_env(mut self, auth_env: AuthEnvTelemetryMetadata) -> Self {
        if let Some(metadata) = self.metadata.as_mut() {
            metadata.auth_env = auth_env;
        }
        self
    }

    pub fn with_model(mut self, model: &str, slug: &str) -> Self {
        if let Some(metadata) = self.metadata.as_mut() {
            metadata.model = model.to_string();
            metadata.slug = slug.to_string();
        }
        self
    }

    pub fn with_metrics_service_name(mut self, service_name: &str) -> Self {
        if let Some(metadata) = self.metadata.as_mut() {
            metadata.service_name = Some(sanitize_metric_tag_value(service_name));
        }
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
        if let Some(metrics) = &self.metrics {
            let _ = metrics.counter(name, inc, tags);
        }
    }

    pub fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) {
        if let Some(metrics) = &self.metrics {
            let _ = metrics.histogram(name, value, tags);
        }
    }

    pub fn record_duration(&self, name: &str, duration: Duration, tags: &[(&str, &str)]) {
        if let Some(metrics) = &self.metrics {
            let _ = metrics.record_duration(name, duration, tags);
        }
    }

    pub fn start_timer(&self, name: &str, tags: &[(&str, &str)]) -> Result<Timer, MetricsError> {
        let Some(metrics) = &self.metrics else {
            return Err(MetricsError::ExporterDisabled);
        };
        metrics.start_timer(name, tags)
    }

    pub fn shutdown_metrics(&self) -> MetricsResult<()> {
        if let Some(metrics) = &self.metrics {
            metrics.shutdown()
        } else {
            Ok(())
        }
    }

    pub fn snapshot_metrics(&self) -> MetricsResult<crate::metrics::data::ResourceMetrics> {
        if let Some(metrics) = &self.metrics {
            metrics.snapshot()
        } else {
            Ok(crate::metrics::data::ResourceMetrics)
        }
    }

    pub fn reset_runtime_metrics(&self) {
        if let Some(metrics) = &self.metrics {
            metrics.reset_runtime_metrics();
        }
    }

    pub fn runtime_metrics_summary(&self) -> Option<RuntimeMetricsSummary> {
        self.metrics
            .as_ref()
            .and_then(MetricsClient::runtime_metrics_summary)
            .or(Some(RuntimeMetricsSummary::default()))
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
            metadata: Some(SessionTelemetryMetadata {
                conversation_id,
                auth_mode: auth_mode.map(|mode| mode.to_string()),
                auth_env: AuthEnvTelemetryMetadata::default(),
                account_id,
                account_email,
                originator: sanitize_metric_tag_value(originator.as_str()),
                service_name: None,
                session_source: session_source.to_string(),
                model: model.to_string(),
                slug: slug.to_string(),
                log_user_prompts,
                app_version: env!("CARGO_PKG_VERSION"),
                terminal_type,
            }),
            metrics: crate::metrics::global(),
            metrics_use_metadata_tags: true,
        }
    }

    pub fn record_responses<T>(&self, _handle_responses_span: &Span, _event: &T) {}

    #[allow(clippy::too_many_arguments)]
    pub fn conversation_starts(
        &self,
        _provider_name: &str,
        _reasoning_effort: Option<ReasoningEffort>,
        _reasoning_summary: ReasoningSummary,
        _context_window: Option<i64>,
        _auto_compact_token_limit: Option<i64>,
        _approval_policy: AskForApproval,
        _sandbox_policy: SandboxPolicy,
        _mcp_servers: Vec<&str>,
        _active_profile: Option<String>,
    ) {
    }

    pub async fn log_request<F, Fut, T, E>(&self, _attempt: u64, f: F) -> Result<T, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        f().await
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_api_request(
        &self,
        _attempt: u64,
        _status: Option<u16>,
        _error: Option<&str>,
        _duration: Duration,
        _auth_header_attached: bool,
        _auth_header_name: Option<&str>,
        _retry_after_unauthorized: bool,
        _recovery_mode: Option<&str>,
        _recovery_phase: Option<&str>,
        _endpoint: &str,
        _request_id: Option<&str>,
        _cf_ray: Option<&str>,
        _auth_error: Option<&str>,
        _auth_error_code: Option<&str>,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_websocket_connect(
        &self,
        _duration: Duration,
        _status: Option<u16>,
        _error: Option<&str>,
        _auth_header_attached: bool,
        _auth_header_name: Option<&str>,
        _retry_after_unauthorized: bool,
        _recovery_mode: Option<&str>,
        _recovery_phase: Option<&str>,
        _endpoint: &str,
        _connection_reused: bool,
        _request_id: Option<&str>,
        _cf_ray: Option<&str>,
        _auth_error: Option<&str>,
        _auth_error_code: Option<&str>,
    ) {
    }

    pub fn record_websocket_request(
        &self,
        _duration: Duration,
        _error: Option<&str>,
        _connection_reused: bool,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_auth_recovery(
        &self,
        _mode: &str,
        _step: &str,
        _outcome: &str,
        _request_id: Option<&str>,
        _cf_ray: Option<&str>,
        _auth_error: Option<&str>,
        _auth_error_code: Option<&str>,
        _recovery_reason: Option<&str>,
        _auth_state_changed: Option<bool>,
    ) {
    }

    pub fn record_websocket_event<T>(&self, _result: &T, _duration: Duration) {}

    pub fn log_sse_event<E>(
        &self,
        _response: &Result<
            Option<Result<eventsource_stream::Event, eventsource_stream::EventStreamError<E>>>,
            tokio::time::error::Elapsed,
        >,
        _duration: Duration,
    ) where
        E: std::fmt::Display,
    {
    }

    pub fn sse_event_failed<T>(&self, _kind: Option<&String>, _duration: Duration, _error: &T)
    where
        T: std::fmt::Display,
    {
    }

    pub fn see_event_completed_failed<T>(&self, _error: &T)
    where
        T: std::fmt::Display,
    {
    }

    pub fn sse_event_completed(
        &self,
        _input_token_count: i64,
        _output_token_count: i64,
        _cached_token_count: Option<i64>,
        _reasoning_token_count: Option<i64>,
        _tool_token_count: i64,
    ) {
    }

    pub fn user_prompt(&self, _items: &[UserInput]) {}

    pub fn tool_decision(
        &self,
        _tool_name: &str,
        _call_id: &str,
        _decision: &ReviewDecision,
        _source: ToolDecisionSource,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn log_tool_result_with_tags<F, Fut, E>(
        &self,
        _tool_name: &str,
        _call_id: &str,
        _arguments: &str,
        _extra_tags: &[(&str, &str)],
        _mcp_server: Option<&str>,
        _mcp_server_origin: Option<&str>,
        f: F,
    ) -> Result<(String, bool), E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(String, bool), E>>,
        E: std::fmt::Display,
    {
        f().await
    }

    pub fn log_tool_failed(&self, _tool_name: &str, _error: &str) {}

    #[allow(clippy::too_many_arguments)]
    pub fn tool_result_with_tags(
        &self,
        _tool_name: &str,
        _call_id: &str,
        _arguments: &str,
        _duration: Duration,
        _success: bool,
        _output: &str,
        _extra_tags: &[(&str, &str)],
        _mcp_server: Option<&str>,
        _mcp_server_origin: Option<&str>,
    ) {
    }
}

#[derive(Clone, Debug, Default)]
pub struct NoopTracingLayer<S>(PhantomData<S>);

impl<S> Layer<S> for NoopTracingLayer<S>
where
    S: tracing::Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_new_span(
        &self,
        _attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::Id,
        _ctx: LayerContext<'_, S>,
    ) {
    }
}

pub fn start_global_timer(name: &str, tags: &[(&str, &str)]) -> MetricsResult<Timer> {
    let Some(metrics) = crate::metrics::global() else {
        return Err(MetricsError::ExporterDisabled);
    };
    metrics.start_timer(name, tags)
}
