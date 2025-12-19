use crate::config::OtelExporter;
use crate::config::OtelHttpProtocol;
use crate::config::OtelSettings;
use crate::config::OtelTlsConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use http::Uri;
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
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_TRACES_TIMEOUT;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_otlp::WithTonicConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::BatchSpanProcessor;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::Tracer;
use opentelemetry_semantic_conventions as semconv;
use reqwest::Certificate as ReqwestCertificate;
use reqwest::Identity as ReqwestIdentity;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::io::{self};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tonic::metadata::MetadataMap;
use tonic::transport::Certificate as TonicCertificate;
use tonic::transport::ClientTlsConfig;
use tonic::transport::Identity as TonicIdentity;
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
}

impl OtelProvider {
    pub fn shutdown(&self) {
        if let Some(logger) = &self.logger {
            let _ = logger.shutdown();
        }
        if let Some(tracer_provider) = &self.tracer_provider {
            let _ = tracer_provider.shutdown();
        }
    }

    pub fn from(settings: &OtelSettings) -> Result<Option<Self>, Box<dyn Error>> {
        let log_enabled = !matches!(settings.exporter, OtelExporter::None);
        let trace_enabled = !matches!(settings.trace_exporter, OtelExporter::None);

        if !log_enabled && !trace_enabled {
            debug!("No exporter enabled in OTLP settings.");
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
            let _ = global::set_tracer_provider(provider);
            global::set_text_map_propagator(TraceContextPropagator::new());
        }
        if tracer.is_some() {
            attach_traceparent_context();
        }

        Ok(Some(Self {
            logger,
            tracer_provider,
            tracer,
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
}

impl Drop for OtelProvider {
    fn drop(&mut self) {
        if let Some(logger) = &self.logger {
            let _ = logger.shutdown();
        }
        if let Some(tracer_provider) = &self.tracer_provider {
            let _ = tracer_provider.shutdown();
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

    match exporter {
        OtelExporter::None => return Ok(builder.build()),
        OtelExporter::OtlpGrpc {
            endpoint,
            headers,
            tls,
        } => {
            debug!("Using OTLP Grpc exporter: {endpoint}");

            let header_map = build_header_map(headers);

            let base_tls_config = ClientTlsConfig::new()
                .with_enabled_roots()
                .assume_http2(true);

            let tls_config = match tls.as_ref() {
                Some(tls) => build_grpc_tls_config(endpoint, base_tls_config, tls)?,
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
                .with_headers(headers.clone());

            if let Some(tls) = tls.as_ref() {
                let client = build_http_client(tls, OTEL_EXPORTER_OTLP_LOGS_TIMEOUT)?;
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
    let span_exporter = match exporter {
        OtelExporter::None => return Ok(SdkTracerProvider::builder().build()),
        OtelExporter::OtlpGrpc {
            endpoint,
            headers,
            tls,
        } => {
            debug!("Using OTLP Grpc exporter for traces: {endpoint}");

            let header_map = build_header_map(headers);

            let base_tls_config = ClientTlsConfig::new()
                .with_enabled_roots()
                .assume_http2(true);

            let tls_config = match tls.as_ref() {
                Some(tls) => build_grpc_tls_config(endpoint, base_tls_config, tls)?,
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
                .with_headers(headers.clone());

            if let Some(tls) = tls.as_ref() {
                let client = build_http_client(tls, OTEL_EXPORTER_OTLP_TRACES_TIMEOUT)?;
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

fn build_header_map(headers: &HashMap<String, String>) -> HeaderMap {
    let mut header_map = HeaderMap::new();
    for (key, value) in headers {
        if let Ok(name) = HeaderName::from_bytes(key.as_bytes())
            && let Ok(val) = HeaderValue::from_str(value)
        {
            header_map.insert(name, val);
        }
    }
    header_map
}

fn build_grpc_tls_config(
    endpoint: &str,
    tls_config: ClientTlsConfig,
    tls: &OtelTlsConfig,
) -> Result<ClientTlsConfig, Box<dyn Error>> {
    let uri: Uri = endpoint.parse()?;
    let host = uri.host().ok_or_else(|| {
        config_error(format!(
            "OTLP gRPC endpoint {endpoint} does not include a host"
        ))
    })?;

    let mut config = tls_config.domain_name(host.to_owned());

    if let Some(path) = tls.ca_certificate.as_ref() {
        let (pem, _) = read_bytes(path)?;
        config = config.ca_certificate(TonicCertificate::from_pem(pem));
    }

    match (&tls.client_certificate, &tls.client_private_key) {
        (Some(cert_path), Some(key_path)) => {
            let (cert_pem, _) = read_bytes(cert_path)?;
            let (key_pem, _) = read_bytes(key_path)?;
            config = config.identity(TonicIdentity::from_pem(cert_pem, key_pem));
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(config_error(
                "client_certificate and client_private_key must both be provided for mTLS",
            ));
        }
        (None, None) => {}
    }

    Ok(config)
}

/// Build a blocking HTTP client with TLS configuration for the OTLP HTTP exporter.
///
/// We use `reqwest::blocking::Client` instead of the async client because the
/// `opentelemetry_sdk` `BatchLogProcessor` spawns a dedicated OS thread that uses
/// `futures_executor::block_on()` rather than tokio. When the async reqwest client's
/// timeout calls `tokio::time::sleep()`, it panics with "no reactor running".
fn build_http_client(
    tls: &OtelTlsConfig,
    timeout_var: &str,
) -> Result<reqwest::blocking::Client, Box<dyn Error>> {
    // Wrap in block_in_place because reqwest::blocking::Client creates its own
    // internal tokio runtime, which would panic if built directly from an async context.
    tokio::task::block_in_place(|| build_http_client_inner(tls, timeout_var))
}

fn build_http_client_inner(
    tls: &OtelTlsConfig,
    timeout_var: &str,
) -> Result<reqwest::blocking::Client, Box<dyn Error>> {
    let mut builder =
        reqwest::blocking::Client::builder().timeout(resolve_otlp_timeout(timeout_var));

    if let Some(path) = tls.ca_certificate.as_ref() {
        let (pem, location) = read_bytes(path)?;
        let certificate = ReqwestCertificate::from_pem(pem.as_slice()).map_err(|error| {
            config_error(format!(
                "failed to parse certificate {}: {error}",
                location.display()
            ))
        })?;
        // Disable built-in root certificates and use only our custom CA
        builder = builder
            .tls_built_in_root_certs(false)
            .add_root_certificate(certificate);
    }

    match (&tls.client_certificate, &tls.client_private_key) {
        (Some(cert_path), Some(key_path)) => {
            let (mut cert_pem, cert_location) = read_bytes(cert_path)?;
            let (key_pem, key_location) = read_bytes(key_path)?;
            cert_pem.extend_from_slice(key_pem.as_slice());
            let identity = ReqwestIdentity::from_pem(cert_pem.as_slice()).map_err(|error| {
                config_error(format!(
                    "failed to parse client identity using {} and {}: {error}",
                    cert_location.display(),
                    key_location.display()
                ))
            })?;
            builder = builder.identity(identity).https_only(true);
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(config_error(
                "client_certificate and client_private_key must both be provided for mTLS",
            ));
        }
        (None, None) => {}
    }

    builder
        .build()
        .map_err(|error| Box::new(error) as Box<dyn Error>)
}

fn resolve_otlp_timeout(signal_var: &str) -> Duration {
    if let Some(timeout) = read_timeout_env(signal_var) {
        return timeout;
    }
    if let Some(timeout) = read_timeout_env(OTEL_EXPORTER_OTLP_TIMEOUT) {
        return timeout;
    }
    OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT
}

fn read_timeout_env(var: &str) -> Option<Duration> {
    let value = env::var(var).ok()?;
    let parsed = value.parse::<i64>().ok()?;
    if parsed < 0 {
        return None;
    }
    Some(Duration::from_millis(parsed as u64))
}

fn read_bytes(path: &AbsolutePathBuf) -> Result<(Vec<u8>, PathBuf), Box<dyn Error>> {
    match fs::read(path) {
        Ok(bytes) => Ok((bytes, path.to_path_buf())),
        Err(error) => Err(Box::new(io::Error::new(
            error.kind(),
            format!("failed to read {}: {error}", path.display()),
        ))),
    }
}

fn config_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidData, message.into()))
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
