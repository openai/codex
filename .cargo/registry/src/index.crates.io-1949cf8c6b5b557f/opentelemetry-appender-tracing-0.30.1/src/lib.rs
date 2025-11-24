//! # OpenTelemetry-Appender-Tracing
//!
//! This crate provides a bridge between the [`tracing`](https://docs.rs/tracing/latest/tracing/) crate and OpenTelemetry logs.
//! It converts `tracing` events into OpenTelemetry `LogRecords`, allowing applications using `tracing` to seamlessly integrate
//! with OpenTelemetry logging backends.
//!
//! ## Background
//!
//! Unlike traces and metrics, OpenTelemetry does not provide a dedicated logging API for end-users. Instead, it recommends using
//! existing logging libraries and bridging them to OpenTelemetry logs. This crate serves as such a bridge for `tracing` users.
//!
//! ## Features
//!
//! - Converts `tracing` events into OpenTelemetry [`LogRecords`](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/logs/data-model.md#log-and-event-record-definition)
//! - Integrates as a [`Layer`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/trait.Layer.html)
//!   from [`tracing-subscriber`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/), allowing
//!   to be used alongside other `tracing` layers, such as `fmt`
//! - Automatically attaches OpenTelemetry trace context (`TraceId`, `SpanId`, `TraceFlags`) to logs
//! - Automatically associates OpenTelemetry Resource to logs
//! - Supports exporting logs to OpenTelemetry-compatible backends (OTLP, stdout, etc.)
//!
//! ## Getting Started
//!
//! ### 1. Install Dependencies
//!
//! Add the following dependencies to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! tracing = ">=0.1.40"
//! tracing-core = { version = ">=0.1.33" }
//! tracing-subscriber = { version = "0.3", features = ["registry", "std", "fmt"] }
//! opentelemetry = { version = "0.28", features = ["logs"] }
//! opentelemetry-sdk = { version = "0.28", features = ["logs"] }
//! opentelemetry-appender-tracing = { version = "0.28.1" }
//! ```
//!
//! ### 2. Set Up the OpenTelemetry Logger Provider
//!
//! Before integrating with `tracing`, create an OpenTelemetry [`SdkLoggerProvider`](https://docs.rs/opentelemetry_sdk/latest/opentelemetry_sdk/logs/struct.SdkLoggerProvider.html):
//!
//! ```rust
//! use opentelemetry_sdk::logs::SdkLoggerProvider;
//! use opentelemetry_stdout::LogExporter;
//!
//! let exporter = LogExporter::default();
//! let provider = SdkLoggerProvider::builder()
//!     .with_simple_exporter(exporter)
//!     .build();
//! ```
//!
//! In this example, `SdkLoggerProvider` is configured to use the `opentelemetry_stdout` crate to export logs to stdout. You can replace it with any other OpenTelemetry-compatible exporter.
//! Any additional OpenTelemetry configuration (e.g., setting up a resource, additional processors etc.) can be done at this stage.
//!
//! ### 3. Create the OpenTelemetry-Tracing Bridge
//!
//! Create `OpenTelemetryTracingBridge` layer using the `SdkLoggerProvider` created in the previous step.
//!
//! ```rust
//! # use opentelemetry_sdk::logs::SdkLoggerProvider;
//! # use opentelemetry_stdout::LogExporter;
//! # use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
//! # let exporter = LogExporter::default();
//! # let provider = SdkLoggerProvider::builder()
//! #    .with_simple_exporter(exporter)
//! #    .build();
//! let otel_layer = OpenTelemetryTracingBridge::new(&provider);
//! ```
//!
//! ### 4. Register the `tracing` Subscriber
//!
//! Since this crate provides a `Layer` for `tracing`, you can register it with the `tracing` subscriber as shown below.
//!
//! ```rust
//! # use opentelemetry_sdk::logs::SdkLoggerProvider;
//! # use opentelemetry_stdout::LogExporter;
//! # let exporter = LogExporter::default();
//! # let provider = SdkLoggerProvider::builder().with_simple_exporter(exporter).build();
//! # use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
//! # let otel_layer = OpenTelemetryTracingBridge::new(&provider);
//! use tracing_subscriber::prelude::*;
//!
//! tracing_subscriber::registry()
//!     .with(otel_layer)
//!     .with(tracing_subscriber::fmt::layer()) // In this example, `fmt` layer is also added.
//!     .init();
//! ```
//!
//! ### 5. Log Events Using `tracing`
//!
//! ```rust
//! use tracing::error;
//! error!(name: "my-event-name1", target: "my-system", event_id = 10, user_name = "otel", user_email = "otel@opentelemetry.io", message = "This is an example message");
//! ```
//!
//!
//! ## Mapping details
//!
//! Since OpenTelemetry and `tracing` have their own data models, this bridge performs the following mappings:
//!
//! | `tracing`             | OpenTelemetry           | Notes                                                                                   |
//! |-----------------------|-------------------------|-----------------------------------------------------------------------------------------|
//! | name of the event     | `EventName`             | OpenTelemetry defines logs with name as Events, so every `tracing` Event is actually an OTel Event |
//! | target                | `target`                | Groups logs from the same module/crate. At recording time, `target` is stored in a top-level field. But exporters treat this information as OpenTelemetry `InstrumentationScope` |
//! | level of the event    | `Severity`, `SeverityText` |                                                                                         |
//! | Fields                | `Attributes`            | Converted into OpenTelemetry log attributes. Field with "message" as key is specially treated and stored as `LogRecord::Body` |
//! | Message               | `Body`                  | The body/message of the log. This is done only if body was not already populated from "message" field above |
//!
//! ### Data Type Mapping
//!
//! The data types supported by `tracing` and OpenTelemetry are different and the following conversions are applied:
//!
//! | `tracing` Type | OpenTelemetry `AnyValue` Type |
//! |----------------|-------------------------------|
//! | `i64`          | `Int`                         |
//! | `f32`, `f64`   | `Double`                      |
//! | `u64`,`u128` ,`i128`         | `Int` (if convertible to `i64` without loss) else `String` |
//! | `&str`         | `String`                      |
//! | `bool`         | `Bool`                        |
//! | `&[u8]`        | `Bytes`                       |
//! | `&dyn Debug`   | `String` (via `Debug` formatting) |
//! | `&dyn Error`   | `String` (via `Debug` formatting). This is stored into an attribute with key "exception.message", following [OTel conventions](https://opentelemetry.io/docs/specs/semconv/attributes-registry/exception/) |
//!
//! In future, additional types may be supported.
//!
//! > **Note:** This crate does not support `tracing` Spans. One may use [`tracing-opentelemetry`](https://docs.rs/tracing-opentelemetry/latest/tracing_opentelemetry/) to
//! > convert `tracing` spans into OpenTelemetry spans. This is a third-party crate
//! > that is not maintained by the OpenTelemetry project.
//! > `tracing-opentelemetry`:
//! > - Converts `tracing` spans into OpenTelemetry spans  
//! > - Converts `tracing` events into OpenTelemetry `SpanEvents` rather than logs
//! >   Depending on the outcome of the
//! >   [discussion](https://github.com/open-telemetry/opentelemetry-rust/issues/1571),
//! >   the OpenTelemetry project may provide direct support to map `tracing`
//! >   spans to OpenTelemetry in the future.
//!
//! ## Feature Flags
//! `spec_unstable_logs_enabled`: TODO
//!
//! `experimental_metadata_attributes`: TODO
//!
//! `experimental_use_tracing_span_context`: TODO
//!
//! ## Limitations
//! 1. There is no support for `Valuable` crate. [2819](https://github.com/open-telemetry/opentelemetry-rust/issues/2819)
//!
//! ## Stability Guarantees
//! // TODO
//!
//! ## Further Reading
//!
//! - OpenTelemetry Rust: [opentelemetry-rust](https://github.com/open-telemetry/opentelemetry-rust)
//! - Tracing: [tracing](https://docs.rs/tracing/)
//! - OpenTelemetry Logs: [OpenTelemetry Logging Specification](https://opentelemetry.io/docs/specs/otel/logs/)
pub mod layer;
