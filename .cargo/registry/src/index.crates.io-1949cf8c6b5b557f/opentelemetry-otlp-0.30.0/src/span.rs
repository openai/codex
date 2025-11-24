//! # OTLP - Span Exporter
//!
//! Defines a [SpanExporter] to send trace data via the OpenTelemetry Protocol (OTLP)

use std::fmt::Debug;

use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::SpanData;

use crate::ExporterBuildError;
#[cfg(feature = "grpc-tonic")]
use crate::{
    exporter::tonic::{HasTonicConfig, TonicExporterBuilder},
    TonicExporterBuilderSet,
};

#[cfg(any(feature = "http-proto", feature = "http-json"))]
use crate::{
    exporter::http::{HasHttpConfig, HttpExporterBuilder},
    HttpExporterBuilderSet,
};

use crate::{exporter::HasExportConfig, NoExporterBuilderSet};

/// Target to which the exporter is going to send spans, defaults to https://localhost:4317/v1/traces.
/// Learn about the relationship between this constant and default/metrics/logs at
/// <https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/protocol/exporter.md#endpoint-urls-for-otlphttp>
pub const OTEL_EXPORTER_OTLP_TRACES_ENDPOINT: &str = "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT";
/// Max waiting time for the backend to process each spans batch, defaults to 10s.
pub const OTEL_EXPORTER_OTLP_TRACES_TIMEOUT: &str = "OTEL_EXPORTER_OTLP_TRACES_TIMEOUT";
/// Compression algorithm to use, defaults to none.
pub const OTEL_EXPORTER_OTLP_TRACES_COMPRESSION: &str = "OTEL_EXPORTER_OTLP_TRACES_COMPRESSION";
/// Key-value pairs to be used as headers associated with gRPC or HTTP requests
/// for sending spans.
/// Example: `k1=v1,k2=v2`
/// Note: this is only supported for HTTP.
pub const OTEL_EXPORTER_OTLP_TRACES_HEADERS: &str = "OTEL_EXPORTER_OTLP_TRACES_HEADERS";

/// OTLP span exporter builder
#[derive(Debug, Default, Clone)]
pub struct SpanExporterBuilder<C> {
    client: C,
}

impl SpanExporterBuilder<NoExporterBuilderSet> {
    /// Create a new [SpanExporterBuilder] with default settings.
    pub fn new() -> Self {
        SpanExporterBuilder::default()
    }

    /// With the gRPC Tonic transport.
    #[cfg(feature = "grpc-tonic")]
    pub fn with_tonic(self) -> SpanExporterBuilder<TonicExporterBuilderSet> {
        SpanExporterBuilder {
            client: TonicExporterBuilderSet(TonicExporterBuilder::default()),
        }
    }

    /// With the HTTP transport.
    #[cfg(any(feature = "http-proto", feature = "http-json"))]
    pub fn with_http(self) -> SpanExporterBuilder<HttpExporterBuilderSet> {
        SpanExporterBuilder {
            client: HttpExporterBuilderSet(HttpExporterBuilder::default()),
        }
    }
}

#[cfg(feature = "grpc-tonic")]
impl SpanExporterBuilder<TonicExporterBuilderSet> {
    /// Build the [SpanExporter] with the gRPC Tonic transport.
    pub fn build(self) -> Result<SpanExporter, ExporterBuildError> {
        let span_exporter = self.client.0.build_span_exporter()?;
        opentelemetry::otel_debug!(name: "SpanExporterBuilt");
        Ok(span_exporter)
    }
}

#[cfg(any(feature = "http-proto", feature = "http-json"))]
impl SpanExporterBuilder<HttpExporterBuilderSet> {
    /// Build the [SpanExporter] with the HTTP transport.
    pub fn build(self) -> Result<SpanExporter, ExporterBuildError> {
        let span_exporter = self.client.0.build_span_exporter()?;
        Ok(span_exporter)
    }
}

#[cfg(feature = "grpc-tonic")]
impl HasExportConfig for SpanExporterBuilder<TonicExporterBuilderSet> {
    fn export_config(&mut self) -> &mut crate::ExportConfig {
        &mut self.client.0.exporter_config
    }
}

#[cfg(any(feature = "http-proto", feature = "http-json"))]
impl HasExportConfig for SpanExporterBuilder<HttpExporterBuilderSet> {
    fn export_config(&mut self) -> &mut crate::ExportConfig {
        &mut self.client.0.exporter_config
    }
}

#[cfg(feature = "grpc-tonic")]
impl HasTonicConfig for SpanExporterBuilder<TonicExporterBuilderSet> {
    fn tonic_config(&mut self) -> &mut crate::TonicConfig {
        &mut self.client.0.tonic_config
    }
}

#[cfg(any(feature = "http-proto", feature = "http-json"))]
impl HasHttpConfig for SpanExporterBuilder<HttpExporterBuilderSet> {
    fn http_client_config(&mut self) -> &mut crate::exporter::http::HttpConfig {
        &mut self.client.0.http_config
    }
}

/// OTLP exporter that sends tracing data
#[derive(Debug)]
pub struct SpanExporter {
    client: SupportedTransportClient,
}

#[derive(Debug)]
enum SupportedTransportClient {
    #[cfg(feature = "grpc-tonic")]
    Tonic(crate::exporter::tonic::trace::TonicTracesClient),
    #[cfg(any(feature = "http-proto", feature = "http-json"))]
    Http(crate::exporter::http::OtlpHttpClient),
}

impl SpanExporter {
    /// Obtain a builder to configure a [SpanExporter].
    pub fn builder() -> SpanExporterBuilder<NoExporterBuilderSet> {
        SpanExporterBuilder::default()
    }

    #[cfg(any(feature = "http-proto", feature = "http-json"))]
    pub(crate) fn from_http(client: crate::exporter::http::OtlpHttpClient) -> Self {
        SpanExporter {
            client: SupportedTransportClient::Http(client),
        }
    }

    #[cfg(feature = "grpc-tonic")]
    pub(crate) fn from_tonic(client: crate::exporter::tonic::trace::TonicTracesClient) -> Self {
        SpanExporter {
            client: SupportedTransportClient::Tonic(client),
        }
    }
}

impl opentelemetry_sdk::trace::SpanExporter for SpanExporter {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        match &self.client {
            #[cfg(feature = "grpc-tonic")]
            SupportedTransportClient::Tonic(client) => client.export(batch).await,
            #[cfg(any(feature = "http-proto", feature = "http-json"))]
            SupportedTransportClient::Http(client) => client.export(batch).await,
        }
    }

    fn set_resource(&mut self, resource: &opentelemetry_sdk::Resource) {
        match &mut self.client {
            #[cfg(feature = "grpc-tonic")]
            SupportedTransportClient::Tonic(client) => client.set_resource(resource),
            #[cfg(any(feature = "http-proto", feature = "http-json"))]
            SupportedTransportClient::Http(client) => client.set_resource(resource),
        }
    }
}
