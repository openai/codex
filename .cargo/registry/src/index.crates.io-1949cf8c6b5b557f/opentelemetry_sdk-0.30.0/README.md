# OpenTelemetry Rust SDK

![OpenTelemetry — An observability framework for cloud-native software.][splash]

[splash]: https://raw.githubusercontent.com/open-telemetry/opentelemetry-rust/main/assets/logo-text.png

This crate contains the [OpenTelemetry](https://opentelemetry.io/) SDK
implementation for Rust.

[![Crates.io: opentelemetry-sdk](https://img.shields.io/crates/v/opentelemetry_sdk.svg)](https://crates.io/crates/opentelemetry_sdk)
[![Documentation](https://docs.rs/opentelemetry_sdk/badge.svg)](https://docs.rs/opentelemetry_sdk)
[![LICENSE](https://img.shields.io/crates/l/opentelemetry_sdk)](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-sdk/LICENSE)
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

This crate is official SDK implementation of OpenTelemetry encompassing several
aspects of OpenTelemetry, such as context management and propagation, logging,
tracing, and metrics. It follows the [OpenTelemetry
specification](https://github.com/open-telemetry/opentelemetry-specification).
Here's a breakdown of its components:

- **[Propagators
  Implementation](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/context/api-propagators.md):**
  While the `opentelemetry` crate contained the API, this crate contains the actual implementation.
- **[Logs SDK](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/logs/sdk.md):**
  Implements the Logs SDK specification.
- **[Tracing
  SDK](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/sdk.md):**
  Implements the Tracing SDK specification.
- **[Metrics
  SDK](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/sdk.md):**
  Implements the Metrics SDK specification.

This crate lights up the telemetry, by replacing the facade or no-op
implementation from `opentelemetry` crate. In many ways, one can think of
`opentelemetry` as the crate containing the "traits" along with a no-op
implementation, and this (`opentelemetry-sdk`) crate containing a real
implementation to replace the default no-ops.

This crate defines the telemetry pipeline, and makes telemetry available for
processors etc., but the actual exporting of telemetry requires additional
crates, such as
[opentelemetry-stdout](https://crates.io/crates/opentelemetry-stdout),
[opentelemetry-otlp](https://crates.io/crates/opentelemetry-otlp) etc.

### Related crates

Unless you are a plugin (custom Samplers, Processors etc.) author, you will almost always need to use additional
crates along with this. Given this crate has no exporting capability, an
OpenTelemetry Exporter is almost always required. OpenTelemetry provides the following exporters:

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
  specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/sdk_exporters/zipkin.md).
- **[opentelemetry-prometheus](https://crates.io/crates/opentelemetry-prometheus):**
  Exports telemetry (metrics only) to Prometheus following [OpenTelemetry to
  Prometheus
  specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/sdk_exporters/prometheus.md).

OpenTelemetry Rust also has a [contrib
repo](https://github.com/open-telemetry/opentelemetry-rust-contrib), where
additional exporters could be found. Check [OpenTelemetry
Registry](https://opentelemetry.io/ecosystem/registry/?language=rust) for
additional exporters and other related components as well.

## Getting started

See [docs](https://docs.rs/opentelemetry-sdk).

## Release Notes

You can find the release notes (changelog) [here](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-sdk/CHANGELOG.md).

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
