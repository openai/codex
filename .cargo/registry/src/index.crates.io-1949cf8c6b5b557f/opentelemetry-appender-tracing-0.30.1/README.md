# OpenTelemetry Log Appender for `tracing` crate

![OpenTelemetry — An observability framework for cloud-native software.][splash]

[splash]: https://raw.githubusercontent.com/open-telemetry/opentelemetry-rust/main/assets/logo-text.png

This crate contains a [Log
Appender](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/glossary.md#log-appender--bridge)
that bridges logs from the [tracing crate](https://tracing.rs/tracing/#events)
to OpenTelemetry. Note that this is different from the existing
[tracing-opentelemetry](https://github.com/tokio-rs/tracing-opentelemetry)
project, which supports bridging traces and logs from tracing into OpenTelemetry
traces.

[![Crates.io: opentelemetry-appender-tracing](https://img.shields.io/crates/v/opentelemetry-appender-tracing.svg)](https://crates.io/crates/opentelemetry-appender-tracing)
[![Documentation](https://docs.rs/opentelemetry-appender-tracing/badge.svg)](https://docs.rs/opentelemetry-appender-tracing)
[![LICENSE](https://img.shields.io/crates/l/opentelemetry-appender-tracing)](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-appender-tracing/LICENSE)
[![GitHub Actions CI](https://github.com/open-telemetry/opentelemetry-rust/workflows/CI/badge.svg)](https://github.com/open-telemetry/opentelemetry-rust/actions?query=workflow%3ACI+branch%3Amain)
[![Slack](https://img.shields.io/badge/slack-@cncf/otel/rust-brightgreen.svg?logo=slack)](https://cloud-native.slack.com/archives/C03GDP0H023)

## OpenTelemetry Overview

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

## Release Notes

You can find the release notes (changelog) [here](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-appender-tracing/CHANGELOG.md).

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
