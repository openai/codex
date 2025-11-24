//! Implements the [`API`] component of [OpenTelemetry].
//!
//! *[Supported Rust Versions](#supported-rust-versions)*
//!
//! [`API`]: https://opentelemetry.io/docs/specs/otel/overview/#api
//! [OpenTelemetry]: https://opentelemetry.io/docs/what-is-opentelemetry/
//!
//! # Getting Started with Traces
//!
//! The [`trace`] module includes types for tracking the progression of a single
//! request while it is handled by services that make up an application. A trace
//! is a tree of [`Span`]s which are objects that represent the work being done
//! by individual services or components involved in a request as it flows
//! through a system.
//!
//! ```
//! # #[cfg(feature = "trace")]
//! # {
//! use opentelemetry::{global, trace::{Span, Tracer}, KeyValue};
//!
//! // get a tracer from a provider
//! let tracer = global::tracer("my_service");
//!
//! // start a new span
//! let mut span = tracer.start("my_span");
//!
//! // set some attributes
//! span.set_attribute(KeyValue::new("http.client_ip", "83.164.160.102"));
//!
//! // perform some more work...
//!
//! // end or drop the span to export
//! span.end();
//! # }
//! ```
//!
//! See the [examples](https://github.com/open-telemetry/opentelemetry-rust/tree/main/examples) directory for different integration patterns.
//!
//! See the [`trace`] module docs for more information on creating and managing
//! spans.
//!
//! [`Span`]: crate::trace::Span
//!
//! # Getting Started with Metrics
//!
//! The [`metrics`] module provides types for recording measurements about a
//! service at runtime. Below are the key steps to report measurements using
//! OpenTelemetry Metrics:
//!
//! 1. **Obtain a Meter:** Get a `Meter` from a `MeterProvider`.
//! 2. **Create Instruments:** Use the `Meter` to create one or more instruments
//!    (e.g., counters, histograms).
//! 3. **Record Measurements:** Use the instruments to record measurement values
//!    along with optional attributes.
//!
//! ## How Metrics work in OpenTelemetry
//! In OpenTelemetry, raw measurements recorded using instruments are
//! **aggregated in memory** to form metrics. These aggregated metrics are
//! periodically exported by the [`opentelemetry_sdk`] at fixed intervals (e.g.,
//! every 60 seconds) via exporters such as [`opentelemetry-stdout`] or
//! [`opentelemetry-otlp`]. This reduces reporting overhead while ensuring
//! up-to-date data. The aggregation strategy and export interval can be
//! customized in the [`opentelemetry_sdk`] based on your use case.
//!
//! ## Choosing the Right Instrument
//! Selecting the correct instrument is critical for accurately representing
//! your metrics data:
//!
//! - Use **Counters** for values that only increase, such as the number of
//!   requests served or errors encountered.
//! - Use **UpDownCounters** for values that can increase or decrease, such as
//!   the number of active connections, number of items in a queue etc.
//! - **Gauges:** Use for values that can go up or down and represent the
//!   current state, such as CPU usage, temperature etc.
//! - Use **Histograms** for measuring the distribution of a value, such as
//!   response times or payload sizes.
//!
//! ### Observable Instruments
//!
//! Counters, UpDownCounters, and Gauges have Observable variants that allow
//! values to be reported through a callback function. Observable instruments
//! are ideal when the metric value is managed elsewhere and needs to be
//! observed by OpenTelemetry instrumentation. The callbacks are automatically
//! invoked by the OpenTelemetry SDK before every export (e.g., every 60
//! seconds).
//!
//! For example:
//! - An **ObservableCounter** can monitor the number of page faults in a
//!   process as reported by the operating system.
//! - An **ObservableUpDownCounter** can monitor the size of an in-memory queue
//!   by reporting the size using queue's len() method within the callback
//!   function.
//! - An **ObservableGauge** can monitor the CPU temperature by using
//!   temperature sensor APIs within the callback function.
//!   
//! For detailed guidance, refer to [OpenTelemetry Metrics API - Instrumentation
//! Guidance](https://opentelemetry.io/docs/specs/otel/metrics/supplementary-guidelines/#instrument-selection).
//!
//! ## Best Practices
//! - **Re-use Instruments:** Instruments are designed for
//!   reuse. Avoid creating new instruments repeatedly.
//! - **Clone for Sharing:** If the same instrument needs to be used across
//!   multiple parts of your code, you can safely clone it to share.
//!
//! ## Example Usage
//! ```
//! use opentelemetry::{global, KeyValue};
//!
//! // Get a meter from a provider.
//! let meter = global::meter("my_service");
//!
//! // Create an instrument (in this case, a Counter).
//! let counter = meter.u64_counter("request.count").build();
//!
//! // Record a measurement by passing the value and a set of attributes.
//! counter.add(1, &[KeyValue::new("http.client_ip", "83.164.160.102")]);
//!
//! // Create an ObservableCounter and register a callback that reports the measurement.
//! let _observable_counter = meter
//! .u64_observable_counter("bytes_received")
//! .with_callback(|observer| {
//!     observer.observe(
//!         100,
//!         &[
//!             KeyValue::new("protocol", "udp"),
//!         ],
//!     )
//! })
//! .build();
//! ```
//!
//! See the
//! [examples](https://github.com/open-telemetry/opentelemetry-rust/tree/main/examples/metrics-basic)
//! directory that show a runnable example with all type of instruments.
//!
//!
//! See the [`metrics`] module docs for more information on creating and
//! managing instruments.
//!
//!
//! # Getting Started with Logs
//!
//!  The [`logs`] module contains the Logs Bridge API. It is not intended to be
//!  called by application developers directly. It is provided for logging
//!  library authors to build log appenders, that bridges existing logging
//!  systems with OpenTelemetry. Bridges for
//!  [`log`](https://crates.io/crates/log) and
//!  [`tracing`](https://crates.io/crates/tracing) libraries are provided via
//!  the
//!  [`opentelemetry-appender-log`](https://crates.io/crates/opentelemetry-appender-log)
//!  and
//!  [`opentelemetry-appender-tracing`](https://crates.io/crates/opentelemetry-appender-tracing)
//!  crates.
//!
//! # Crate Feature Flags
//!
//! The following core crate feature flags are available:
//!
//! * `trace`: Includes the trace API.
//! * `metrics`: Includes the metrics API.
//! * `logs`: Includes the logs bridge API.
//! * `internal-logs`: Includes internal logging for the OpenTelemetry library via `tracing`.
//!
//! The default feature flags are ["trace", "metrics", "logs", "internal-logs"].
//!
//! The following feature flags provides additional configuration for `logs`:
//! * `spec_unstable_logs_enabled`: Allow users to control the log level
//!
//! The following feature flags enable APIs defined in OpenTelemetry specification that is in experimental phase:
//! * `otel_unstable`: Includes unstable APIs. There are no features behind this flag at the moment.
//!
//! # Related Crates
//!
//! In addition to `opentelemetry`, the [`open-telemetry/opentelemetry-rust`]
//! repository contains several additional crates designed to be used with the
//! `opentelemetry` ecosystem. This includes exporters, samplers, as well as
//! utility and adapter crates to assist in propagating context and
//! instrumenting applications.
//!
//! In particular, the following crates are likely to be of interest:
//!
//! - [`opentelemetry_sdk`] provides the OpenTelemetry SDK used to configure providers.
//! - [`opentelemetry-http`] provides an interface for injecting and extracting
//!   trace information from [`http`] headers.
//! - [`opentelemetry-otlp`] exporter for sending telemetry in the
//!   OTLP format.
//! - [`opentelemetry-stdout`] provides ability to output telemetry to stdout,
//!   primarily used for learning/debugging purposes.
//! - [`opentelemetry-prometheus`] provides a pipeline and exporter for sending
//!   metrics information to [`Prometheus`].
//! - [`opentelemetry-zipkin`] provides a pipeline and exporter for sending
//!   trace information to [`Zipkin`].
//!
//!  In addition, there are several other useful crates in the [OTel Rust
//!  Contrib
//!  repo](https://github.com/open-telemetry/opentelemetry-rust-contrib). A lot
//!  of crates maintained outside OpenTelemetry owned repos can be found in the
//!  [OpenTelemetry
//!  Registry](https://opentelemetry.io/ecosystem/registry/?language=rust).
//!
//! [`http`]: https://crates.io/crates/http
//! [`open-telemetry/opentelemetry-rust`]: https://github.com/open-telemetry/opentelemetry-rust
//! [`opentelemetry_sdk`]: https://crates.io/crates/opentelemetry_sdk
//! [`opentelemetry-stdout`]: https://crates.io/crates/opentelemetry_stdout
//! [`opentelemetry-http`]: https://crates.io/crates/opentelemetry-http
//! [`opentelemetry-otlp`]: https://crates.io/crates/opentelemetry-otlp
//! [`opentelemetry-prometheus`]: https://crates.io/crates/opentelemetry-prometheus
//! [`opentelemetry-zipkin`]: https://crates.io/crates/opentelemetry-zipkin
//! [`Prometheus`]: https://prometheus.io
//! [`Zipkin`]: https://zipkin.io
//!
//! # Supported Rust Versions
//!
//! OpenTelemetry is built against the latest stable release. The minimum
//! supported version is 1.70. The current OpenTelemetry version is not
//! guaranteed to build on Rust versions earlier than the minimum supported
//! version.
//!
//! The current stable Rust compiler and the three most recent minor versions
//! before it will always be supported. For example, if the current stable
//! compiler version is 1.49, the minimum supported version will not be
//! increased past 1.46, three minor versions prior. Increasing the minimum
//! supported compiler version is not considered a semver breaking change as
//! long as doing so complies with this policy.
#![warn(
    future_incompatible,
    missing_debug_implementations,
    missing_docs,
    nonstandard_style,
    rust_2018_idioms,
    unreachable_pub,
    unused
)]
#![allow(clippy::needless_doctest_main)]
#![cfg_attr(
    docsrs,
    feature(doc_cfg, doc_auto_cfg),
    deny(rustdoc::broken_intra_doc_links)
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/open-telemetry/opentelemetry-rust/main/assets/logo.svg"
)]
#![cfg_attr(test, deny(warnings))]

pub mod global;

pub mod baggage;

pub mod context;

pub use context::{Context, ContextGuard};

mod trace_context;
pub use trace_context::{SpanId, TraceFlags, TraceId};

mod common;

#[cfg(any(feature = "testing", test))]
#[doc(hidden)]
pub mod testing;

pub use common::{
    Array, InstrumentationScope, InstrumentationScopeBuilder, Key, KeyValue, StringValue, Value,
};

#[cfg(feature = "metrics")]
#[cfg_attr(docsrs, doc(cfg(feature = "metrics")))]
pub mod metrics;

#[cfg(feature = "trace")]
#[cfg_attr(docsrs, doc(cfg(feature = "trace")))]
pub mod propagation;

#[cfg(feature = "trace")]
#[cfg_attr(docsrs, doc(cfg(feature = "trace")))]
pub mod trace;

#[cfg(feature = "logs")]
#[cfg_attr(docsrs, doc(cfg(feature = "logs")))]
pub mod logs;

#[doc(hidden)]
#[cfg(any(feature = "metrics", feature = "trace", feature = "logs"))]
pub mod time {
    use std::time::SystemTime;

    #[doc(hidden)]
    #[cfg(any(
        not(target_arch = "wasm32"),
        all(target_arch = "wasm32", target_os = "wasi")
    ))]
    pub fn now() -> SystemTime {
        SystemTime::now()
    }

    #[doc(hidden)]
    #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
    pub fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(js_sys::Date::now() as u64)
    }
}

#[doc(hidden)]
pub mod _private {
    #[cfg(feature = "internal-logs")]
    pub use tracing::{debug, error, info, warn}; // re-export
}
