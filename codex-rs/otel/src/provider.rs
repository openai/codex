use crate::config::OtelExporter;
use crate::config::OtelHttpProtocol;
use crate::config::OtelSettings;
use crate::config::StatsigMetricsSettings;
use crate::metrics::MetricsClient;
use crate::metrics::MetricsConfig;
use crate::targets::is_log_export_target;
use crate::targets::is_trace_safe_target;
use gethostname::gethostname;
use opentelemetry::Context;
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::trace::Span as _;
use opentelemetry::trace::SpanBuilder;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::LogExporter;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_LOGS_TIMEOUT;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_TRACES_TIMEOUT;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_otlp::WithTonicConfig;
use opentelemetry_otlp::tonic_types::metadata::MetadataMap;
use opentelemetry_otlp::tonic_types::transport::ClientTlsConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::runtime;
use opentelemetry_sdk::trace::BatchSpanProcessor;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::Span;
use opentelemetry_sdk::trace::SpanData;
use opentelemetry_sdk::trace::SpanProcessor;
use opentelemetry_sdk::trace::Tracer;
use opentelemetry_sdk::trace::TracerProviderBuilder;
use opentelemetry_sdk::trace::span_processor_with_async_runtime::BatchSpanProcessor as TokioBatchSpanProcessor;
use opentelemetry_semantic_conventions as semconv;
use std::collections::BTreeMap;
use std::error::Error;
use std::io;
use std::mem::ManuallyDrop;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;
use tracing::debug;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;

const ENV_ATTRIBUTE: &str = "env";
const HOST_NAME_ATTRIBUTE: &str = "host.name";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResourceKind {
    Logs,
    Traces,
}

pub struct OtelProvider {
    pub logger: Option<SdkLoggerProvider>,
    pub tracer_provider: Option<SdkTracerProvider>,
    pub tracer: Option<Tracer>,
    pub metrics: Option<MetricsClient>,
    shutdown_started: AtomicBool,
    shutdown_worker: Option<mpsc::SyncSender<ShutdownWorker>>,
}

struct ShutdownWorker {
    provider: ManuallyDrop<OtelProvider>,
    completed_tx: tokio::sync::oneshot::Sender<()>,
}

struct ShutdownWorkerStartup {
    worker_rx: mpsc::Receiver<ShutdownWorker>,
    ready_tx: mpsc::SyncSender<()>,
}

#[derive(Debug)]
struct GlobalTracer {
    service_name: &'static str,
}

impl opentelemetry::trace::Tracer for GlobalTracer {
    type Span = global::BoxedSpan;

    fn build_with_context(&self, builder: SpanBuilder, parent: &Context) -> Self::Span {
        global::tracer(self.service_name).build_with_context(builder, parent)
    }
}

impl OtelProvider {
    /// Flushes and shuts down configured exporters at most once.
    pub fn shutdown(&self) {
        if self.shutdown_started.swap(/*val*/ true, Ordering::AcqRel) {
            return;
        }

        if let Some(tracer_provider) = &self.tracer_provider {
            let _ = tracer_provider.shutdown();
        }
        if let Some(metrics) = &self.metrics {
            let _ = metrics.shutdown();
        }
        if let Some(logger) = &self.logger {
            let _ = logger.shutdown();
        }
    }

    /// Starts the detached shutdown worker before shutdown-time resource pressure.
    fn prepare_shutdown_worker(&mut self) -> io::Result<()> {
        self.prepare_shutdown_worker_with_spawner(|startup| {
            std::thread::Builder::new()
                .name("codex-otel-shutdown".to_string())
                .spawn(move || {
                    if startup.ready_tx.send(()).is_err() {
                        return;
                    }
                    let Ok(worker) = startup.worker_rx.recv() else {
                        return;
                    };
                    let provider = ManuallyDrop::into_inner(worker.provider);
                    provider.shutdown();
                    drop(provider);
                    let _ = worker.completed_tx.send(());
                })
        })
    }

    fn prepare_shutdown_worker_with_spawner<F>(&mut self, spawn: F) -> io::Result<()>
    where
        F: FnOnce(ShutdownWorkerStartup) -> io::Result<std::thread::JoinHandle<()>>,
    {
        if self.shutdown_worker.is_some() {
            return Ok(());
        }

        let (worker_tx, worker_rx) = mpsc::sync_channel(/*bound*/ 1);
        let (ready_tx, ready_rx) = mpsc::sync_channel(/*bound*/ 1);
        let startup = ShutdownWorkerStartup {
            worker_rx,
            ready_tx,
        };
        let _shutdown_worker = spawn(startup)?;
        ready_rx.recv().map_err(|_| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "telemetry shutdown worker stopped before initializing",
            )
        })?;
        self.shutdown_worker = Some(worker_tx);
        Ok(())
    }

    /// Shuts down exporters on a prepared detached thread within a time budget.
    pub async fn shutdown_with_timeout(mut self, timeout: Duration) -> io::Result<()> {
        let Some(worker_tx) = self.shutdown_worker.take() else {
            // Best-effort shutdown must not run a potentially blocking destructor
            // when its worker could not be prepared.
            let _provider = ManuallyDrop::new(self);
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "telemetry shutdown worker was not initialized",
            ));
        };

        let (completed_tx, completed_rx) = tokio::sync::oneshot::channel();
        let worker = ShutdownWorker {
            provider: ManuallyDrop::new(self),
            completed_tx,
        };
        worker_tx.send(worker).map_err(|_| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "telemetry shutdown worker stopped before receiving the provider",
            )
        })?;

        match tokio::time::timeout(timeout, completed_rx).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "telemetry shutdown worker stopped before completing",
            )),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "telemetry shutdown exceeded its time budget",
            )),
        }
    }

    pub fn try_new(settings: &OtelSettings) -> Result<Option<Self>, Box<dyn Error>> {
        let log_enabled = !matches!(settings.exporter, OtelExporter::None);
        let trace_enabled = !matches!(settings.trace_exporter, OtelExporter::None);
        let metric_exporter = crate::config::resolve_exporter(&settings.metrics_exporter);
        let metrics_enabled = !matches!(metric_exporter, OtelExporter::None);

        if !log_enabled && !trace_enabled && !metrics_enabled {
            // Tracestate propagation is process-global; clear it when these
            // settings do not install an active provider.
            crate::trace_context::set_tracestate_entries(BTreeMap::new())?;
            debug!("No OTEL exporter enabled in settings.");
            return Ok(None);
        }

        // Provider setup installs process-global OTEL state that cannot be
        // rolled back. Validate trace metadata before any setup path can
        // mutate those globals, and keep span attribute checks aligned with
        // config loading when traces are exported.
        if trace_enabled {
            crate::config::validate_span_attributes(&settings.span_attributes)?;
        }
        crate::trace_context::validate_tracestate_entries(&settings.tracestate)?;

        let metrics = if matches!(metric_exporter, OtelExporter::None) {
            None
        } else {
            let mut config = MetricsConfig::otlp(
                settings.environment.clone(),
                settings.service_name.clone(),
                settings.service_version.clone(),
                settings.metrics_exporter.clone(),
            );
            if settings.runtime_metrics {
                config = config.with_runtime_reader();
            }
            Some(MetricsClient::new(config)?)
        };

        let log_resource = make_resource(settings, ResourceKind::Logs);
        let trace_resource = make_resource(settings, ResourceKind::Traces);
        let logger = log_enabled
            .then(|| build_logger(&log_resource, &settings.exporter))
            .transpose()?;

        let tracer_provider = trace_enabled
            .then(|| {
                build_tracer_provider(
                    &trace_resource,
                    &settings.trace_exporter,
                    settings.span_attributes.clone(),
                )
            })
            .transpose()?;

        let tracer = tracer_provider
            .as_ref()
            .map(|provider| provider.tracer(settings.service_name.clone()));

        let mut provider = Self {
            logger,
            tracer_provider,
            tracer,
            metrics,
            shutdown_started: AtomicBool::default(),
            shutdown_worker: None,
        };
        provider.prepare_shutdown_worker()?;

        crate::trace_context::set_tracestate_entries(settings.tracestate.clone())?;
        if let Some(tracer_provider) = provider.tracer_provider.clone() {
            global::set_tracer_provider(tracer_provider);
            global::set_text_map_propagator(TraceContextPropagator::new());
        }
        if let Some(metrics) = provider.metrics.as_mut() {
            *metrics = crate::metrics::install_global(metrics.clone());
            if matches!(settings.metrics_exporter, OtelExporter::Statsig) {
                crate::metrics::install_global_statsig_settings(StatsigMetricsSettings {
                    environment: settings.environment.clone(),
                });
            }
        }
        Ok(Some(provider))
    }

    pub fn logger_layer<S>(&self) -> Option<impl Layer<S> + Send + Sync>
    where
        S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
    {
        self.logger_export_layer().map(|layer| {
            layer.with_filter(tracing_subscriber::filter::filter_fn(
                OtelProvider::log_export_filter,
            ))
        })
    }

    /// Returns a log-export bridge that must be installed beneath the log export filter.
    pub fn logger_export_layer<S>(&self) -> Option<impl Layer<S> + Send + Sync>
    where
        S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
    {
        self.logger.as_ref().map(OpenTelemetryTracingBridge::new)
    }

    pub fn tracing_layer<S>(&self) -> Option<impl Layer<S> + Send + Sync>
    where
        S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
    {
        self.tracer.as_ref().map(|tracer| {
            tracing_opentelemetry::layer()
                .with_tracer(tracer.clone())
                .with_filter(tracing_subscriber::filter::filter_fn(
                    OtelProvider::trace_export_filter,
                ))
        })
    }

    /// Returns a permanent trace layer that follows the process-global tracer provider.
    pub fn reloadable_tracing_layer<S>(service_name: &'static str) -> impl Layer<S> + Send + Sync
    where
        S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
    {
        tracing_opentelemetry::layer()
            .with_tracer(GlobalTracer { service_name })
            .with_filter(tracing_subscriber::filter::filter_fn(
                Self::trace_export_filter,
            ))
    }

    pub fn codex_export_filter(meta: &tracing::Metadata<'_>) -> bool {
        Self::log_export_filter(meta)
    }

    pub fn log_export_filter(meta: &tracing::Metadata<'_>) -> bool {
        is_log_export_target(meta.target())
    }

    pub fn trace_export_filter(meta: &tracing::Metadata<'_>) -> bool {
        meta.is_span() || is_trace_safe_target(meta.target())
    }

    pub fn metrics(&self) -> Option<&MetricsClient> {
        self.metrics.as_ref()
    }
}

impl Drop for OtelProvider {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn make_resource(settings: &OtelSettings, kind: ResourceKind) -> Resource {
    Resource::builder()
        .with_service_name(settings.service_name.clone())
        .with_attributes(resource_attributes(
            settings,
            detected_host_name().as_deref(),
            kind,
        ))
        .build()
}

fn resource_attributes(
    settings: &OtelSettings,
    host_name: Option<&str>,
    kind: ResourceKind,
) -> Vec<KeyValue> {
    let mut attributes = vec![
        KeyValue::new(
            semconv::attribute::SERVICE_VERSION,
            settings.service_version.clone(),
        ),
        KeyValue::new(ENV_ATTRIBUTE, settings.environment.clone()),
    ];
    if kind == ResourceKind::Logs
        && let Some(host_name) = host_name.and_then(normalize_host_name)
    {
        attributes.push(KeyValue::new(HOST_NAME_ATTRIBUTE, host_name));
    }
    attributes
}

fn detected_host_name() -> Option<String> {
    let host_name = gethostname();
    normalize_host_name(host_name.to_string_lossy().as_ref())
}

fn normalize_host_name(host_name: &str) -> Option<String> {
    let host_name = host_name.trim();
    (!host_name.is_empty()).then(|| host_name.to_owned())
}

fn tracer_provider_builder(
    resource: &Resource,
    span_attributes: BTreeMap<String, String>,
) -> TracerProviderBuilder {
    let builder = SdkTracerProvider::builder().with_resource(resource.clone());
    if span_attributes.is_empty() {
        builder
    } else {
        builder.with_span_processor(SpanAttributesProcessor {
            attributes: span_attributes,
        })
    }
}

/// Applies configured attributes when spans start.
///
/// Resource attributes describe the provider process. These attributes are
/// per-span metadata, so they need to be attached before each span is exported.
#[derive(Debug)]
struct SpanAttributesProcessor {
    attributes: BTreeMap<String, String>,
}

impl SpanProcessor for SpanAttributesProcessor {
    fn on_start(&self, span: &mut Span, _cx: &Context) {
        for (key, value) in self.attributes.iter() {
            span.set_attribute(KeyValue::new(key.clone(), value.clone()));
        }
    }

    fn on_end(&self, _span: SpanData) {}

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        Ok(())
    }
}

fn build_logger(
    resource: &Resource,
    exporter: &OtelExporter,
) -> Result<SdkLoggerProvider, Box<dyn Error>> {
    let mut builder = SdkLoggerProvider::builder().with_resource(resource.clone());

    match crate::config::resolve_exporter(exporter) {
        OtelExporter::None => return Ok(builder.build()),
        OtelExporter::Statsig => unreachable!("statsig exporter should be resolved"),
        OtelExporter::OtlpGrpc {
            endpoint,
            headers,
            tls,
        } => {
            debug!("Using OTLP Grpc exporter: {endpoint}");

            let header_map = crate::otlp::build_header_map(&headers);

            let base_tls_config = ClientTlsConfig::new()
                .with_enabled_roots()
                .assume_http2(true);

            let tls_config = match tls.as_ref() {
                Some(tls) => crate::otlp::build_grpc_tls_config(&endpoint, base_tls_config, tls)?,
                None => base_tls_config,
            };

            let exporter = LogExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .with_metadata(MetadataMap::from_headers(header_map))
                .with_tls_config(tls_config)
                .build()?;

            builder = builder.with_batch_exporter(exporter);
        }
        OtelExporter::OtlpHttp {
            endpoint,
            headers,
            protocol,
            tls,
        } => {
            debug!("Using OTLP Http exporter: {endpoint}");

            let protocol = match protocol {
                OtelHttpProtocol::Binary => Protocol::HttpBinary,
                OtelHttpProtocol::Json => Protocol::HttpJson,
            };

            let mut exporter_builder = LogExporter::builder()
                .with_http()
                .with_endpoint(endpoint)
                .with_protocol(protocol)
                .with_headers(headers);

            if let Some(tls) = tls.as_ref() {
                let client = crate::otlp::build_http_client(tls, OTEL_EXPORTER_OTLP_LOGS_TIMEOUT)?;
                exporter_builder = exporter_builder.with_http_client(client);
            }

            let exporter = exporter_builder.build()?;

            builder = builder.with_batch_exporter(exporter);
        }
    }

    Ok(builder.build())
}

fn build_tracer_provider(
    resource: &Resource,
    exporter: &OtelExporter,
    span_attributes: BTreeMap<String, String>,
) -> Result<SdkTracerProvider, Box<dyn Error>> {
    let span_exporter = match crate::config::resolve_exporter(exporter) {
        OtelExporter::None => return Ok(tracer_provider_builder(resource, span_attributes).build()),
        OtelExporter::Statsig => unreachable!("statsig exporter should be resolved"),
        OtelExporter::OtlpGrpc {
            endpoint,
            headers,
            tls,
        } => {
            debug!("Using OTLP Grpc exporter for traces: {endpoint}");

            let header_map = crate::otlp::build_header_map(&headers);

            let base_tls_config = ClientTlsConfig::new()
                .with_enabled_roots()
                .assume_http2(true);

            let tls_config = match tls.as_ref() {
                Some(tls) => crate::otlp::build_grpc_tls_config(&endpoint, base_tls_config, tls)?,
                None => base_tls_config,
            };

            SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .with_metadata(MetadataMap::from_headers(header_map))
                .with_tls_config(tls_config)
                .build()?
        }
        OtelExporter::OtlpHttp {
            endpoint,
            headers,
            protocol,
            tls,
        } => {
            debug!("Using OTLP Http exporter for traces: {endpoint}");

            if crate::otlp::current_tokio_runtime_is_multi_thread() {
                let protocol = match protocol {
                    OtelHttpProtocol::Binary => Protocol::HttpBinary,
                    OtelHttpProtocol::Json => Protocol::HttpJson,
                };

                let mut exporter_builder = SpanExporter::builder()
                    .with_http()
                    .with_endpoint(endpoint)
                    .with_protocol(protocol)
                    .with_headers(headers);

                let client = crate::otlp::build_async_http_client(
                    tls.as_ref(),
                    OTEL_EXPORTER_OTLP_TRACES_TIMEOUT,
                )?;
                exporter_builder = exporter_builder.with_http_client(client);

                let processor =
                    TokioBatchSpanProcessor::builder(exporter_builder.build()?, runtime::Tokio)
                        .build();

                return Ok(tracer_provider_builder(resource, span_attributes)
                    .with_span_processor(processor)
                    .build());
            }

            let protocol = match protocol {
                OtelHttpProtocol::Binary => Protocol::HttpBinary,
                OtelHttpProtocol::Json => Protocol::HttpJson,
            };

            let mut exporter_builder = SpanExporter::builder()
                .with_http()
                .with_endpoint(endpoint)
                .with_protocol(protocol)
                .with_headers(headers);

            if let Some(tls) = tls.as_ref() {
                let client =
                    crate::otlp::build_http_client(tls, OTEL_EXPORTER_OTLP_TRACES_TIMEOUT)?;
                exporter_builder = exporter_builder.with_http_client(client);
            }

            exporter_builder.build()?
        }
    };

    let processor = BatchSpanProcessor::builder(span_exporter).build();

    Ok(tracer_provider_builder(resource, span_attributes)
        .with_span_processor(processor)
        .build())
}

#[cfg(test)]
#[path = "provider_shutdown_tests.rs"]
mod shutdown_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::API_CALL_COUNT_METRIC;
    use crate::metrics::API_CALL_DURATION_METRIC;
    use crate::metrics::MetricsExporter;
    use crate::metrics::RESPONSES_API_ENGINE_IAPI_TTFT_DURATION_METRIC;
    use crate::metrics::RESPONSES_API_ENGINE_SERVICE_TBT_DURATION_METRIC;
    use crate::metrics::RESPONSES_API_ENGINE_SERVICE_TTFT_DURATION_METRIC;
    use crate::metrics::TOOL_CALL_COUNT_METRIC;
    use crate::metrics::TOOL_CALL_DURATION_METRIC;
    use crate::metrics::TURN_TOKEN_USAGE_METRIC;
    use opentelemetry_sdk::metrics::InMemoryMetricExporter;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn resource_attributes_include_host_name_when_present() {
        let attrs = resource_attributes(
            &test_otel_settings(),
            Some("opentelemetry-test"),
            ResourceKind::Logs,
        );

        let host_name = attrs
            .iter()
            .find(|kv| kv.key.as_str() == HOST_NAME_ATTRIBUTE)
            .map(|kv| kv.value.as_str().to_string());

        assert_eq!(host_name, Some("opentelemetry-test".to_string()));
    }

    #[test]
    fn resource_attributes_omit_host_name_when_missing_or_empty() {
        let missing = resource_attributes(
            &test_otel_settings(),
            /*host_name*/ None,
            ResourceKind::Logs,
        );
        let empty = resource_attributes(&test_otel_settings(), Some("   "), ResourceKind::Logs);
        let trace_attrs = resource_attributes(
            &test_otel_settings(),
            Some("opentelemetry-test"),
            ResourceKind::Traces,
        );

        assert!(
            !missing
                .iter()
                .any(|kv| kv.key.as_str() == HOST_NAME_ATTRIBUTE)
        );
        assert!(
            !empty
                .iter()
                .any(|kv| kv.key.as_str() == HOST_NAME_ATTRIBUTE)
        );
        assert!(
            !trace_attrs
                .iter()
                .any(|kv| kv.key.as_str() == HOST_NAME_ATTRIBUTE)
        );
    }

    #[test]
    fn log_export_target_excludes_trace_safe_events() {
        assert!(is_log_export_target("codex_otel.log_only"));
        assert!(is_log_export_target("codex_otel.network_proxy"));
        assert!(!is_log_export_target("codex_otel.trace_safe"));
        assert!(!is_log_export_target("codex_otel.trace_safe.debug"));
    }

    #[test]
    fn trace_export_target_only_includes_trace_safe_prefix() {
        assert!(is_trace_safe_target("codex_otel.trace_safe"));
        assert!(is_trace_safe_target("codex_otel.trace_safe.summary"));
        assert!(!is_trace_safe_target("codex_otel.log_only"));
        assert!(!is_trace_safe_target("codex_otel.network_proxy"));
    }

    #[test]
    fn cached_global_metrics_follow_reinstalled_provider() -> Result<(), Box<dyn Error>> {
        let initial =
            crate::metrics::install_global(MetricsClient::new(MetricsConfig::in_memory(
                "test",
                "codex-test",
                env!("CARGO_PKG_VERSION"),
                InMemoryMetricExporter::default(),
            ))?);
        let cached = crate::metrics::global().expect("initial global metrics client");

        let exporter = InMemoryMetricExporter::default();
        let replacement =
            crate::metrics::install_global(MetricsClient::new(MetricsConfig::in_memory(
                "test",
                "codex-test",
                env!("CARGO_PKG_VERSION"),
                exporter.clone(),
            ))?);
        cached.counter("codex.after_transition", /*inc*/ 1, &[])?;
        initial.shutdown()?;
        replacement.shutdown()?;

        let exported_metrics = exporter.get_finished_metrics()?;
        let mut names: Vec<_> = exported_metrics
            .iter()
            .flat_map(opentelemetry_sdk::metrics::data::ResourceMetrics::scope_metrics)
            .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
            .map(opentelemetry_sdk::metrics::data::Metric::name)
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names, vec!["codex.after_transition"]);

        Ok(())
    }

    #[test]
    fn statsig_disabled_metrics_are_not_exported() -> Result<(), Box<dyn Error>> {
        let exporter = InMemoryMetricExporter::default();
        let mut config = MetricsConfig::otlp(
            "test",
            "codex-cli",
            env!("CARGO_PKG_VERSION"),
            OtelExporter::Statsig,
        );
        config.exporter = MetricsExporter::InMemory(exporter.clone());
        let metrics = MetricsClient::new(config)?;

        metrics.counter(API_CALL_COUNT_METRIC, /*inc*/ 1, &[])?;
        metrics.record_duration(API_CALL_DURATION_METRIC, Duration::from_millis(100), &[])?;
        metrics.counter("codex.conversation.turn.count", /*inc*/ 1, &[])?;
        metrics.record_duration(
            RESPONSES_API_ENGINE_IAPI_TTFT_DURATION_METRIC,
            Duration::from_millis(100),
            &[],
        )?;
        metrics.record_duration(
            RESPONSES_API_ENGINE_SERVICE_TBT_DURATION_METRIC,
            Duration::from_millis(100),
            &[],
        )?;
        metrics.record_duration(
            RESPONSES_API_ENGINE_SERVICE_TTFT_DURATION_METRIC,
            Duration::from_millis(100),
            &[],
        )?;
        metrics.counter(TOOL_CALL_COUNT_METRIC, /*inc*/ 1, &[])?;
        metrics.record_duration(TOOL_CALL_DURATION_METRIC, Duration::from_millis(25), &[])?;
        metrics.histogram(TURN_TOKEN_USAGE_METRIC, /*value*/ 100, &[])?;
        metrics.counter("codex.turns", /*inc*/ 1, &[])?;
        metrics.shutdown()?;

        let exported_metrics = exporter.get_finished_metrics()?;
        let mut names: Vec<_> = exported_metrics
            .iter()
            .flat_map(opentelemetry_sdk::metrics::data::ResourceMetrics::scope_metrics)
            .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
            .map(opentelemetry_sdk::metrics::data::Metric::name)
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names, vec!["codex.turns"]);

        Ok(())
    }

    fn test_otel_settings() -> OtelSettings {
        OtelSettings {
            environment: "test".to_string(),
            service_name: "codex-test".to_string(),
            service_version: "0.0.0".to_string(),
            codex_home: PathBuf::from("."),
            exporter: OtelExporter::None,
            trace_exporter: OtelExporter::None,
            metrics_exporter: OtelExporter::None,
            runtime_metrics: false,
            span_attributes: BTreeMap::new(),
            tracestate: BTreeMap::new(),
        }
    }
}
