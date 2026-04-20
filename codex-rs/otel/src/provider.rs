use crate::config::OtelExporter;
use crate::config::OtelHttpProtocol;
use crate::config::OtelSettings;
use crate::metrics::MetricsClient;
use crate::metrics::MetricsConfig;
use crate::targets::is_log_export_target;
use crate::targets::is_trace_safe_target;
use gethostname::gethostname;
use opentelemetry::KeyValue;
use opentelemetry::global;
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
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::runtime;
use opentelemetry_sdk::trace::BatchSpanProcessor;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::Tracer;
use opentelemetry_sdk::trace::span_processor_with_async_runtime::BatchSpanProcessor as TokioBatchSpanProcessor;
use opentelemetry_semantic_conventions as semconv;
use std::error::Error;
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
}

impl OtelProvider {
    pub fn shutdown(&self) {
        if let Some(tracer_provider) = &self.tracer_provider {
            let _ = tracer_provider.force_flush();
            let _ = tracer_provider.shutdown();
        }
        if let Some(metrics) = &self.metrics {
            let _ = metrics.shutdown();
        }
        if let Some(logger) = &self.logger {
            let _ = logger.shutdown();
        }
    }

    pub fn from(settings: &OtelSettings) -> Result<Option<Self>, Box<dyn Error>> {
        let log_enabled = !matches!(settings.exporter, OtelExporter::None);
        let trace_enabled = !matches!(settings.trace_exporter, OtelExporter::None);

        let metric_exporter = crate::config::resolve_exporter(&settings.metrics_exporter);
        let metrics = if matches!(metric_exporter, OtelExporter::None) {
            None
        } else {
            let mut config = MetricsConfig::otlp(
                settings.environment.clone(),
                settings.service_name.clone(),
                settings.service_version.clone(),
                metric_exporter,
            );
            if settings.runtime_metrics {
                config = config.with_runtime_reader();
            }
            Some(MetricsClient::new(config)?)
        };

        if !log_enabled && !trace_enabled && metrics.is_none() {
            crate::metrics::replace_global(/*metrics*/ None);
            debug!("No OTEL exporter enabled in settings.");
            return Ok(None);
        }

        let log_resource = make_resource(settings, ResourceKind::Logs);
        let trace_resource = make_resource(settings, ResourceKind::Traces);
        let logger = log_enabled
            .then(|| build_logger(&log_resource, &settings.exporter))
            .transpose()?;

        let tracer_provider = trace_enabled
            .then(|| build_tracer_provider(&trace_resource, &settings.trace_exporter))
            .transpose()?;

        let tracer = tracer_provider
            .as_ref()
            .map(|provider| provider.tracer(settings.service_name.clone()));

        if let Some(provider) = tracer_provider.clone() {
            global::set_tracer_provider(provider);
            global::set_text_map_propagator(TraceContextPropagator::new());
        }
        crate::metrics::replace_global(metrics.clone());
        Ok(Some(Self {
            logger,
            tracer_provider,
            tracer,
            metrics,
        }))
    }

    pub fn logger_layer<S>(&self) -> Option<impl Layer<S> + Send + Sync>
    where
        S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
    {
        self.logger.as_ref().map(|logger| {
            OpenTelemetryTracingBridge::new(logger).with_filter(
                tracing_subscriber::filter::filter_fn(OtelProvider::log_export_filter),
            )
        })
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
        if let Some(tracer_provider) = &self.tracer_provider {
            let _ = tracer_provider.force_flush();
            let _ = tracer_provider.shutdown();
        }
        if let Some(metrics) = &self.metrics {
            let _ = metrics.shutdown();
        }
        if let Some(logger) = &self.logger {
            let _ = logger.shutdown();
        }
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
) -> Result<SdkTracerProvider, Box<dyn Error>> {
    let span_exporter = match crate::config::resolve_exporter(exporter) {
        OtelExporter::None => return Ok(SdkTracerProvider::builder().build()),
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

                return Ok(SdkTracerProvider::builder()
                    .with_resource(resource.clone())
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

    Ok(SdkTracerProvider::builder()
        .with_resource(resource.clone())
        .with_span_processor(processor)
        .build())
}

#[cfg(test)]
mod tests {
    use super::*;
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
        }
    }
}
