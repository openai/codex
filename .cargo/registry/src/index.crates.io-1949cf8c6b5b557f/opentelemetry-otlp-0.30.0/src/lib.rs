//! # OpenTelemetry OTLP Exporter
//!
//! The OTLP Exporter enables exporting telemetry data (logs, metrics, and traces) in the
//! OpenTelemetry Protocol (OTLP) format to compatible backends. These backends include:
//!
//! - OpenTelemetry Collector
//! - Open-source observability tools (Prometheus, Jaeger, etc.)
//! - Vendor-specific monitoring platforms
//!
//! This crate supports sending OTLP data via:
//! - gRPC
//! - HTTP (binary protobuf or JSON)
//!
//! ## Quickstart with OpenTelemetry Collector
//!
//! ### HTTP Transport (Port 4318)
//!
//! Run the OpenTelemetry Collector:
//!
//! ```shell
//! $ docker run -p 4318:4318 otel/opentelemetry-collector:latest
//! ```
//!
//! Configure your application to export traces via HTTP:
//!
//! ```no_run
//! # #[cfg(all(feature = "trace", feature = "http-proto"))]
//! # {
//! use opentelemetry::global;
//! use opentelemetry::trace::Tracer;
//! use opentelemetry_otlp::Protocol;
//! use opentelemetry_otlp::WithExportConfig;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
//!     // Initialize OTLP exporter using HTTP binary protocol
//!     let otlp_exporter = opentelemetry_otlp::SpanExporter::builder()
//!         .with_http()
//!         .with_protocol(Protocol::HttpBinary)
//!         .build()?;
//!
//!     // Create a tracer provider with the exporter
//!     let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
//!         .with_simple_exporter(otlp_exporter)
//!         .build();
//!
//!     // Set it as the global provider
//!     global::set_tracer_provider(tracer_provider);
//!
//!     // Get a tracer and create spans
//!     let tracer = global::tracer("my_tracer");
//!     tracer.in_span("doing_work", |_cx| {
//!         // Your application logic here...
//!     });
//!
//!     Ok(())
//! # }
//! }
//! ```
//!
//! ### gRPC Transport (Port 4317)
//!
//! Run the OpenTelemetry Collector:
//!
//! ```shell
//! $ docker run -p 4317:4317 otel/opentelemetry-collector:latest
//! ```
//!
//! Configure your application to export traces via gRPC (the tonic client requires a Tokio runtime):
//!
//! - With `[tokio::main]`
//!
//! ```no_run
//! # #[cfg(all(feature = "trace", feature = "grpc-tonic"))]
//! # {
//! use opentelemetry::{global, trace::Tracer};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
//!     // Initialize OTLP exporter using gRPC (Tonic)
//!     let otlp_exporter = opentelemetry_otlp::SpanExporter::builder()
//!         .with_tonic()
//!         .build()?;
//!
//!     // Create a tracer provider with the exporter
//!     let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
//!         .with_simple_exporter(otlp_exporter)
//!         .build();
//!
//!     // Set it as the global provider
//!     global::set_tracer_provider(tracer_provider);
//!
//!     // Get a tracer and create spans
//!     let tracer = global::tracer("my_tracer");
//!     tracer.in_span("doing_work", |_cx| {
//!         // Your application logic here...
//!     });
//!
//!     Ok(())
//! # }
//! }
//! ```
//!
//! - Without `[tokio::main]`
//!
//!  ```no_run
//! # #[cfg(all(feature = "trace", feature = "grpc-tonic"))]
//! # {
//! use opentelemetry::{global, trace::Tracer};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
//!     // Initialize OTLP exporter using gRPC (Tonic)
//!     let rt = tokio::runtime::Runtime::new()?;
//!     let tracer_provider = rt.block_on(async {
//!         let exporter = opentelemetry_otlp::SpanExporter::builder()
//!             .with_tonic()
//!             .build()
//!             .expect("Failed to create span exporter");
//!         opentelemetry_sdk::trace::SdkTracerProvider::builder()
//!             .with_simple_exporter(exporter)
//!             .build()
//!     });
//!
//!     // Set it as the global provider
//!     global::set_tracer_provider(tracer_provider);
//!
//!     // Get a tracer and create spans
//!     let tracer = global::tracer("my_tracer");
//!     tracer.in_span("doing_work", |_cx| {
//!         // Your application logic here...
//!     });
//!
//!     // Ensure the runtime (`rt`) remains active until the program ends
//!     Ok(())
//! # }
//! }
//! ```
//!
//! ## Using with Jaeger
//!
//! Jaeger natively supports the OTLP protocol, making it easy to send traces directly:
//!
//! ```shell
//! $ docker run -p 16686:16686 -p 4317:4317 -e COLLECTOR_OTLP_ENABLED=true jaegertracing/all-in-one:latest
//! ```
//!
//! After running your application configured with the OTLP exporter, view traces at:
//! `http://localhost:16686`
//!
//! ## Using with Prometheus
//!
//! Prometheus natively supports accepting metrics via the OTLP protocol
//! (HTTP/protobuf). You can [run
//! Prometheus](https://prometheus.io/docs/prometheus/latest/installation/) with
//! the following command:
//!
//! ```shell
//! docker run -p 9090:9090 -v ./prometheus.yml:/etc/prometheus/prometheus.yml prom/prometheus --config.file=/etc/prometheus/prometheus.yml --web.enable-otlp-receiver
//! ```
//!
//! (An empty prometheus.yml file is sufficient for this example.)
//!
//! Modify your application to export metrics via OTLP:
//!
//! ```no_run
//! # #[cfg(all(feature = "metrics", feature = "http-proto"))]
//! # {
//! use opentelemetry::global;
//! use opentelemetry::metrics::Meter;
//! use opentelemetry::KeyValue;
//! use opentelemetry_otlp::Protocol;
//! use opentelemetry_otlp::WithExportConfig;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
//!     // Initialize OTLP exporter using HTTP binary protocol
//!     let exporter = opentelemetry_otlp::MetricExporter::builder()
//!         .with_http()
//!         .with_protocol(Protocol::HttpBinary)
//!         .with_endpoint("http://localhost:9090/api/v1/otlp/v1/metrics")
//!         .build()?;
//!
//!     // Create a meter provider with the OTLP Metric exporter
//!     let meter_provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
//!         .with_periodic_exporter(exporter)
//!         .build();
//!     global::set_meter_provider(meter_provider.clone());
//!
//!     // Get a meter
//!     let meter = global::meter("my_meter");
//!
//!     // Create a metric
//!     let counter = meter.u64_counter("my_counter").build();
//!     counter.add(1, &[KeyValue::new("key", "value")]);
//!
//!     // Shutdown the meter provider. This will trigger an export of all metrics.
//!     meter_provider.shutdown()?;
//!
//!     Ok(())
//! # }
//! }
//! ```
//!
//! After running your application configured with the OTLP exporter, view metrics at:
//! `http://localhost:9090`
//! ## Show Logs, Metrics too (TODO)
//!
//! ## Performance
//!
//! For optimal performance, a batch exporting processor is recommended as the simple
//! processor will export each span synchronously on dropping, and is only good
//! for test/debug purposes.
//!
//! ```toml
//! [dependencies]
//! opentelemetry-otlp = { version = "*", features = ["grpc-tonic"] }
//! ```
//!
//! ```no_run
//! # #[cfg(all(feature = "trace", feature = "grpc-tonic"))]
//! # {
//! use opentelemetry::global;
//! use opentelemetry::trace::Tracer;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
//!     // First, create a OTLP exporter builder. Configure it as you need.
//!     let otlp_exporter = opentelemetry_otlp::SpanExporter::builder().with_tonic().build()?;
//!     // Then pass it into provider builder
//!     let _ = opentelemetry_sdk::trace::SdkTracerProvider::builder()
//!         .with_batch_exporter(otlp_exporter)
//!         .build();
//!     let tracer = global::tracer("my_tracer");
//!     tracer.in_span("doing_work", |cx| {
//!         // Traced app logic here...
//!     });
//!
//!     Ok(())
//!   # }
//! }
//! ```
//!
//! [`tokio`]: https://tokio.rs
//!
//! # Feature Flags
//! The following feature flags can enable exporters for different telemetry signals:
//!
//! * `trace`: Includes the trace exporters.
//! * `metrics`: Includes the metrics exporters.
//! * `logs`: Includes the logs exporters.
//!
//! The following feature flags generate additional code and types:
//! * `serialize`: Enables serialization support for type defined in this crate via `serde`.
//!
//! The following feature flags offer additional configurations on gRPC:
//!
//! For users using `tonic` as grpc layer:
//! * `grpc-tonic`: Use `tonic` as grpc layer.
//! * `gzip-tonic`: Use gzip compression for `tonic` grpc layer.
//! * `zstd-tonic`: Use zstd compression for `tonic` grpc layer.
//! * `tls-roots`: Adds system trust roots to rustls-based gRPC clients using the rustls-native-certs crate
//! * `tls-webpki-roots`: Embeds Mozilla's trust roots to rustls-based gRPC clients using the webpki-roots crate
//!
//! The following feature flags offer additional configurations on http:
//!
//! * `http-proto`: Use http as transport layer, protobuf as body format. This feature is enabled by default.
//! * `reqwest-blocking-client`: Use reqwest blocking http client. This feature is enabled by default.
//! * `reqwest-client`: Use reqwest http client.
//! * `reqwest-rustls`: Use reqwest with TLS with system trust roots via `rustls-native-certs` crate.
//! * `reqwest-rustls-webpki-roots`: Use reqwest with TLS with Mozilla's trust roots via `webpki-roots` crate.
//!
//! # Kitchen Sink Full Configuration
//!
//! Example showing how to override all configuration options.
//!
//! Generally there are two parts of configuration. One is the exporter, the other is the provider.
//! Users can configure the exporter using [SpanExporter::builder()] for traces,
//! and [MetricExporter::builder()] + [opentelemetry_sdk::metrics::PeriodicReader::builder()] for metrics.
//! Once you have an exporter, you can add it to either a [opentelemetry_sdk::trace::SdkTracerProvider::builder()] for traces,
//! or [opentelemetry_sdk::metrics::SdkMeterProvider::builder()] for metrics.
//!
//! ```no_run
//! use opentelemetry::{global, KeyValue, trace::Tracer};
//! use opentelemetry_sdk::{trace::{self, RandomIdGenerator, Sampler}, Resource};
//! # #[cfg(feature = "metrics")]
//! use opentelemetry_sdk::metrics::Temporality;
//! use opentelemetry_otlp::{Protocol, WithExportConfig, WithTonicConfig};
//! use std::time::Duration;
//! # #[cfg(feature = "grpc-tonic")]
//! use tonic::metadata::*;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
//!     # #[cfg(all(feature = "trace", feature = "grpc-tonic"))]
//!     # let tracer = {
//!     let mut map = MetadataMap::with_capacity(3);
//!
//!     map.insert("x-host", "example.com".parse().unwrap());
//!     map.insert("x-number", "123".parse().unwrap());
//!     map.insert_bin("trace-proto-bin", MetadataValue::from_bytes(b"[binary data]"));
//!     let exporter = opentelemetry_otlp::SpanExporter::builder()
//!         .with_tonic()
//!         .with_endpoint("http://localhost:4317")
//!         .with_timeout(Duration::from_secs(3))
//!         .with_metadata(map)
//!         .build()?;
//!
//!     let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
//!         .with_batch_exporter(exporter)
//!         .with_sampler(Sampler::AlwaysOn)
//!         .with_id_generator(RandomIdGenerator::default())
//!         .with_max_events_per_span(64)
//!         .with_max_attributes_per_span(16)
//!         .with_resource(Resource::builder_empty().with_attributes([KeyValue::new("service.name", "example")]).build())
//!         .build();
//!     global::set_tracer_provider(tracer_provider.clone());
//!     let tracer = global::tracer("tracer-name");
//!         # tracer
//!     # };
//!
//!     # #[cfg(all(feature = "metrics", feature = "grpc-tonic"))]
//!     # {
//!     let exporter = opentelemetry_otlp::MetricExporter::builder()
//!        .with_tonic()
//!        .with_endpoint("http://localhost:4318/v1/metrics")
//!        .with_protocol(Protocol::Grpc)
//!        .with_timeout(Duration::from_secs(3))
//!        .build()
//!        .unwrap();
//!
//!    let provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
//!         .with_periodic_exporter(exporter)
//!         .with_resource(Resource::builder_empty().with_attributes([KeyValue::new("service.name", "example")]).build())
//!         .build();
//!     # }
//!
//! # #[cfg(all(feature = "trace", feature = "grpc-tonic"))]
//! # {
//!     tracer.in_span("doing_work", |cx| {
//!         // Traced app logic here...
//!     });
//! # }
//!
//!     Ok(())
//! }
//! ```
#![warn(
    future_incompatible,
    missing_debug_implementations,
    missing_docs,
    nonstandard_style,
    rust_2018_idioms,
    unreachable_pub,
    unused
)]
#![allow(elided_lifetimes_in_paths)]
#![cfg_attr(
    docsrs,
    feature(doc_cfg, doc_auto_cfg),
    deny(rustdoc::broken_intra_doc_links)
)]
#![cfg_attr(test, deny(warnings))]

mod exporter;
#[cfg(feature = "logs")]
#[cfg(any(feature = "http-proto", feature = "http-json", feature = "grpc-tonic"))]
mod logs;
#[cfg(feature = "metrics")]
#[cfg(any(feature = "http-proto", feature = "http-json", feature = "grpc-tonic"))]
mod metric;
#[cfg(feature = "trace")]
#[cfg(any(feature = "http-proto", feature = "http-json", feature = "grpc-tonic"))]
mod span;

pub use crate::exporter::Compression;
pub use crate::exporter::ExportConfig;
pub use crate::exporter::ExporterBuildError;
#[cfg(feature = "trace")]
#[cfg(any(feature = "http-proto", feature = "http-json", feature = "grpc-tonic"))]
pub use crate::span::{
    SpanExporter, SpanExporterBuilder, OTEL_EXPORTER_OTLP_TRACES_COMPRESSION,
    OTEL_EXPORTER_OTLP_TRACES_ENDPOINT, OTEL_EXPORTER_OTLP_TRACES_HEADERS,
    OTEL_EXPORTER_OTLP_TRACES_TIMEOUT,
};

#[cfg(feature = "metrics")]
#[cfg(any(feature = "http-proto", feature = "http-json", feature = "grpc-tonic"))]
pub use crate::metric::{
    MetricExporter, MetricExporterBuilder, OTEL_EXPORTER_OTLP_METRICS_COMPRESSION,
    OTEL_EXPORTER_OTLP_METRICS_ENDPOINT, OTEL_EXPORTER_OTLP_METRICS_HEADERS,
    OTEL_EXPORTER_OTLP_METRICS_TIMEOUT,
};

#[cfg(feature = "logs")]
#[cfg(any(feature = "http-proto", feature = "http-json", feature = "grpc-tonic"))]
pub use crate::logs::{
    LogExporter, LogExporterBuilder, OTEL_EXPORTER_OTLP_LOGS_COMPRESSION,
    OTEL_EXPORTER_OTLP_LOGS_ENDPOINT, OTEL_EXPORTER_OTLP_LOGS_HEADERS,
    OTEL_EXPORTER_OTLP_LOGS_TIMEOUT,
};

#[cfg(any(feature = "http-proto", feature = "http-json"))]
pub use crate::exporter::http::{HasHttpConfig, WithHttpConfig};

#[cfg(feature = "grpc-tonic")]
pub use crate::exporter::tonic::{HasTonicConfig, WithTonicConfig};

pub use crate::exporter::{
    HasExportConfig, WithExportConfig, OTEL_EXPORTER_OTLP_COMPRESSION, OTEL_EXPORTER_OTLP_ENDPOINT,
    OTEL_EXPORTER_OTLP_ENDPOINT_DEFAULT, OTEL_EXPORTER_OTLP_HEADERS, OTEL_EXPORTER_OTLP_PROTOCOL,
    OTEL_EXPORTER_OTLP_PROTOCOL_DEFAULT, OTEL_EXPORTER_OTLP_TIMEOUT,
    OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT,
};

/// Type to indicate the builder does not have a client set.
#[derive(Debug, Default, Clone)]
pub struct NoExporterBuilderSet;

/// Type to hold the [TonicExporterBuilder] and indicate it has been set.
///
/// Allowing access to [TonicExporterBuilder] specific configuration methods.
#[cfg(feature = "grpc-tonic")]
// This is for clippy to work with only the grpc-tonic feature enabled
#[allow(unused)]
#[derive(Debug, Default)]
pub struct TonicExporterBuilderSet(TonicExporterBuilder);

/// Type to hold the [HttpExporterBuilder] and indicate it has been set.
///
/// Allowing access to [HttpExporterBuilder] specific configuration methods.
#[cfg(any(feature = "http-proto", feature = "http-json"))]
#[derive(Debug, Default)]
pub struct HttpExporterBuilderSet(HttpExporterBuilder);

#[cfg(any(feature = "http-proto", feature = "http-json"))]
pub use crate::exporter::http::HttpExporterBuilder;

#[cfg(feature = "grpc-tonic")]
pub use crate::exporter::tonic::{TonicConfig, TonicExporterBuilder};

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

/// The communication protocol to use when exporting data.
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Protocol {
    /// GRPC protocol
    Grpc,
    /// HTTP protocol with binary protobuf
    HttpBinary,
    /// HTTP protocol with JSON payload
    HttpJson,
}

#[derive(Debug, Default)]
#[doc(hidden)]
/// Placeholder type when no exporter pipeline has been configured in telemetry pipeline.
pub struct NoExporterConfig(());

/// Re-exported types from the `tonic` crate.
#[cfg(feature = "grpc-tonic")]
pub mod tonic_types {
    /// Re-exported types from `tonic::metadata`.
    pub mod metadata {
        #[doc(no_inline)]
        pub use tonic::metadata::MetadataMap;
    }

    /// Re-exported types from `tonic::transport`.
    #[cfg(feature = "tls")]
    pub mod transport {
        #[doc(no_inline)]
        pub use tonic::transport::{Certificate, ClientTlsConfig, Identity};
    }
}
