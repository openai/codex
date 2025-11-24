# OpenTelemetry Rust API

![OpenTelemetry — An observability framework for cloud-native software.][splash]

[splash]: https://raw.githubusercontent.com/open-telemetry/opentelemetry-rust/main/assets/logo-text.png

This crate contains the [OpenTelemetry](https://opentelemetry.io/) API for Rust.

[![Crates.io: opentelemetry](https://img.shields.io/crates/v/opentelemetry.svg)](https://crates.io/crates/opentelemetry)
[![Documentation](https://docs.rs/opentelemetry/badge.svg)](https://docs.rs/opentelemetry)
[![LICENSE](https://img.shields.io/crates/l/opentelemetry)](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry/LICENSE)
[![GitHub Actions CI](https://github.com/open-telemetry/opentelemetry-rust/workflows/CI/badge.svg)](https://github.com/open-telemetry/opentelemetry-rust/actions?query=workflow%3ACI+branch%3Amain)
[![codecov](https://codecov.io/gh/open-telemetry/opentelemetry-rust/branch/main/graph/badge.svg)](https://codecov.io/gh/open-telemetry/opentelemetry-rust)
[![Slack](https://img.shields.io/badge/slack-@cncf/otel/rust-brightgreen.svg?logo=slack)](https://cloud-native.slack.com/archives/C03GDP0H023)

## Overview

OpenTelemetry is an Observability framework and toolkit designed to create and
manage telemetry data such as traces, metrics, and logs. OpenTelemetry is
vendor- and tool-agnostic, meaning that it can be used with a broad variety of
Observability backends, including open source tools like [Jaeger] and
[Prometheus], as well as commercial offerings.

OpenTelemetry is *not* an observability backend like Jaeger, Prometheus, or other
commercial vendors. OpenTelemetry is focused on the generation, collection,
management, and export of telemetry. A major goal of OpenTelemetry is that you
can easily instrument your applications or systems, no matter their language,
infrastructure, or runtime environment. Crucially, the storage and visualization
of telemetry is intentionally left to other tools.

*[Supported Rust Versions](#supported-rust-versions)*

[Prometheus]: https://prometheus.io
[Jaeger]: https://www.jaegertracing.io

### What does this crate contain?

This crate is basic foundation for integrating OpenTelemetry into libraries and
applications, encompassing several aspects of OpenTelemetry, such as context
management and propagation, baggage, logging, tracing, and metrics. It follows
the [OpenTelemetry
specification](https://github.com/open-telemetry/opentelemetry-specification).
Here's a breakdown of its components:

- **[Context
  API](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/context/README.md):**
  Provides a way to manage and propagate context, which is essential for keeping
  track of trace execution across asynchronous tasks.
- **[Propagators
  API](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/context/api-propagators.md):**
  Defines how context can be shared across process boundaries, ensuring
  continuity across microservices or distributed systems.
- **[Baggage
  API](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/baggage/api.md):**
  Allows for the attachment of metadata (baggage) to telemetry, which can be
  used for sharing application-specific information across service boundaries.
- **[Logs Bridge
  API](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/logs/api.md):**
  Allows to bridge existing logging mechanisms with OpenTelemetry logging. This
  is **NOT** meant for end users to call, instead it is meant to enable writing
  bridges/appenders for existing logging mechanisms such as
  [log](https://crates.io/crates/log) or
  [tracing](https://crates.io/crates/tracing).
- **[Tracing
  API](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/api.md):**
  Offers a set of primitives to produce distributed traces to understand the
  flow of a request across system boundaries.
- **[Metrics
  API](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/api.md):**
  Offers a set of primitives to produce measurements of operational metrics like
  latency, throughput, or error rates.

This crate serves as a facade or no-op implementation, meaning it defines the
traits for instrumentation but does not itself implement the processing or
exporting of telemetry data. This separation of concerns allows library authors
to depend on the API crate without tying themselves to a specific
implementation.

Actual implementation and the heavy lifting of telemetry data collection,
processing, and exporting are delegated to the
[opentelemetry-sdk](https://crates.io/crates/opentelemetry-sdk) crate and
various exporter crates such as
[opentelemetry-otlp](https://crates.io/crates/opentelemetry-otlp). This
architecture ensures that the final application can light up the instrumentation
by integrating an SDK implementation.

Library authors are recommended to depend on this crate *only*. This approach is
also aligned with the design philosophy of existing telemetry solutions in the
Rust ecosystem, like `tracing` or `log`, where these crates only offer a facade
and the actual functionality is enabled through additional crates.

### Related crates

Unless you are a library author, you will almost always need to use additional
crates along with this. Given this crate has no-op implementation only, an
OpenTelemetry SDK is always required.
[opentelemetry-sdk](https://crates.io/crates/opentelemetry-sdk) is the official
SDK implemented by OpenTelemetry itself, though it is possible to use a
different sdk.

Additionally one or more exporters are also required to export telemetry to a
destination. OpenTelemetry provides the following exporters:

- **[opentelemetry-stdout](https://crates.io/crates/opentelemetry-stdout):**
  Prints telemetry to stdout, primarily used for learning/debugging purposes.
- **[opentelemetry-otlp](https://crates.io/crates/opentelemetry-otlp):** Exports
  telemetry (logs, metrics and traces) in the [OTLP
  format](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/protocol)
  to an endpoint accepting OTLP. This could be the [OTel
  Collector](https://github.com/open-telemetry/opentelemetry-collector),
  telemetry backends like [Jaeger](https://www.jaegertracing.io/),
  [Prometheus](https://prometheus.io/docs/prometheus/latest/feature_flags/#otlp-receiver)
  or [vendor specific endpoints](https://opentelemetry.io/ecosystem/vendors/).
- **[opentelemetry-zipkin](https://crates.io/crates/opentelemetry-zipkin):**
  Exports telemetry (traces only) to Zipkin following [OpenTelemetry to Zipkin
  specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/sdk_exporters/zipkin.md)
- **[opentelemetry-prometheus](https://crates.io/crates/opentelemetry-prometheus):**
  Exports telemetry (metrics only) to Prometheus following [OpenTelemetry to
  Prometheus
  specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/sdk_exporters/prometheus.md)

OpenTelemetry Rust also has a [contrib
repo](https://github.com/open-telemetry/opentelemetry-rust-contrib), where
additional exporters could be found. Check [OpenTelemetry
Registry](https://opentelemetry.io/ecosystem/registry/?language=rust) for
additional exporters and other related components as well.

## Getting started

See [docs](https://docs.rs/opentelemetry).

## Release Notes

You can find the release notes (changelog) [here](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry/CHANGELOG.md).

## Supported Rust Versions

OpenTelemetry is built against the latest stable release. The minimum supported
version is 1.75.0. The current OpenTelemetry version is not guaranteed to build
on Rust versions earlier than the minimum supported version.

The current stable Rust compiler and the three most recent minor versions
before it will always be supported. For example, if the current stable compiler
version is 1.49, the minimum supported version will not be increased past 1.46,
three minor versions prior. Increasing the minimum supported compiler version
is not considered a semver breaking change as long as doing so complies with
this policy.
