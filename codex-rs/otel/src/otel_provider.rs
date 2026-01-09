use crate::config::OtelExporter;
use crate::config::OtelHttpProtocol;
use crate::config::OtelSettings;
use crate::metrics::MetricsClient;
use crate::metrics::MetricsConfig;
use opentelemetry::Context;
use opentelemetry::KeyValue;
use opentelemetry::context::ContextGuard;
use opentelemetry::global;
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry::trace::TraceContextExt;
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
use opentelemetry_sdk::trace::BatchSpanProcessor;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::Tracer;
use opentelemetry_semantic_conventions as semconv;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::sync::OnceLock;
use tracing::debug;
use tracing::level_filters::LevelFilter;
use tracing::warn;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;

const ENV_ATTRIBUTE: &str = "env";
const TRACEPARENT_ENV_VAR: &str = "TRACEPARENT";
const TRACESTATE_ENV_VAR: &str = "TRACESTATE";
static TRACEPARENT_CONTEXT: OnceLock<Option<Context>> = OnceLock::new();

thread_local! {
    static TRACEPARENT_GUARD: RefCell<Option<ContextGuard>> = const { RefCell::new(None) };
}
pub struct OtelProvider {
    pub logger: Option<SdkLoggerProvider>,
    pub tracer_provider: Option<SdkTracerProvider>,
    pub tracer: Option<Tracer>,
    pub metrics: Option<MetricsClient>,
}

impl OtelProvider {
    pub fn shutdown(&self) {
        if let Some(logger) = &self.logger {
            let _ = logger.shutdown();
        }
        if let Some(tracer_provider) = &self.tracer_provider {
            let _ = tracer_provider.shutdown();
        }
        if let Some(metrics) = &self.metrics {
            let _ = metrics.shutdown();
        }
    }

    pub fn from(settings: &OtelSettings) -> Result<Option<Self>, Box<dyn Error>> {
        let log_enabled = !matches!(settings.exporter, OtelExporter::None);
        let trace_enabled = !matches!(settings.trace_exporter, OtelExporter::None);

        let metric_exporter = crate::config::resolve_exporter(&settings.metrics_exporter);
        let metrics = if matches!(metric_exporter, OtelExporter::None) {
            None
        } else {
            Some(MetricsClient::new(MetricsConfig::otlp(
                settings.environment.clone(),
                settings.service_name.clone(),
                settings.service_version.clone(),
                metric_exporter,
            ))?)
        };

        if let Some(metrics) = metrics.as_ref() {
            crate::metrics::install_global(metrics.clone());
        }

        if !log_enabled && !trace_enabled && metrics.is_none() {
            debug!("No OTEL exporter enabled in settings.");
            return Ok(None);
        }

        let resource = make_resource(settings);
        let logger = log_enabled
            .then(|| build_logger(&resource, &settings.exporter))
            .transpose()?;

        let tracer_provider = trace_enabled
            .then(|| build_tracer_provider(&resource, &settings.trace_exporter))
            .transpose()?;

        let tracer = tracer_provider
            .as_ref()
            .map(|provider| provider.tracer(settings.service_name.clone()));

        if let Some(provider) = tracer_provider.clone() {
            global::set_tracer_provider(provider);
            global::set_text_map_propagator(TraceContextPropagator::new());
        }
        if tracer.is_some() {
            attach_traceparent_context();
        }

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
                tracing_subscriber::filter::filter_fn(OtelProvider::codex_export_filter),
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
                .with_filter(LevelFilter::TRACE)
        })
    }

    pub fn codex_export_filter(meta: &tracing::Metadata<'_>) -> bool {
        meta.target().starts_with("codex_otel")
    }

    pub fn metrics(&self) -> Option<&MetricsClient> {
        self.metrics.as_ref()
    }
}

impl Drop for OtelProvider {
    fn drop(&mut self) {
        if let Some(logger) = &self.logger {
            let _ = logger.shutdown();
        }
        if let Some(tracer_provider) = &self.tracer_provider {
            let _ = tracer_provider.shutdown();
        }
        if let Some(metrics) = &self.metrics {
            let _ = metrics.shutdown();
        }
    }
}

pub(crate) fn traceparent_context_from_env() -> Option<Context> {
    TRACEPARENT_CONTEXT
        .get_or_init(load_traceparent_context)
        .clone()
}

fn attach_traceparent_context() {
    TRACEPARENT_GUARD.with(|guard| {
        let mut guard = guard.borrow_mut();
        if guard.is_some() {
            return;
        }
        if let Some(context) = traceparent_context_from_env() {
            *guard = Some(context.attach());
        }
    });
}

fn load_traceparent_context() -> Option<Context> {
    let traceparent = env::var(TRACEPARENT_ENV_VAR).ok()?;
    let tracestate = env::var(TRACESTATE_ENV_VAR).ok();

    match extract_traceparent_context(traceparent, tracestate) {
        Some(context) => {
            debug!("TRACEPARENT detected; continuing trace from parent context");
            Some(context)
        }
        None => {
            warn!("TRACEPARENT is set but invalid; ignoring trace context");
            None
        }
    }
}

fn extract_traceparent_context(traceparent: String, tracestate: Option<String>) -> Option<Context> {
    let mut headers = HashMap::new();
    headers.insert("traceparent".to_string(), traceparent);
    if let Some(tracestate) = tracestate {
        headers.insert("tracestate".to_string(), tracestate);
    }

    let context = TraceContextPropagator::new().extract(&headers);
    let span = context.span();
    let span_context = span.span_context();
    if !span_context.is_valid() {
        return None;
    }
    Some(context)
}

fn make_resource(settings: &OtelSettings) -> Resource {
    Resource::builder()
        .with_service_name(settings.service_name.clone())
        .with_attributes(vec![
            KeyValue::new(
                semconv::attribute::SERVICE_VERSION,
                settings.service_version.clone(),
            ),
            KeyValue::new(ENV_ATTRIBUTE, settings.environment.clone()),
        ])
        .build()
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
    use opentelemetry::trace::SpanId;
    use opentelemetry::trace::TraceContextExt;
    use opentelemetry::trace::TraceId;

    #[test]
    fn parses_valid_traceparent() {
        let trace_id = "00000000000000000000000000000001";
        let span_id = "0000000000000002";
        let context = extract_traceparent_context(format!("00-{trace_id}-{span_id}-01"), None)
            .expect("trace context");
        let span = context.span();
        let span_context = span.span_context();
        assert_eq!(
            span_context.trace_id(),
            TraceId::from_hex(trace_id).unwrap()
        );
        assert_eq!(span_context.span_id(), SpanId::from_hex(span_id).unwrap());
        assert!(span_context.is_remote());
    }

    #[test]
    fn invalid_traceparent_returns_none() {
        assert!(extract_traceparent_context("not-a-traceparent".to_string(), None).is_none());
    }
}
