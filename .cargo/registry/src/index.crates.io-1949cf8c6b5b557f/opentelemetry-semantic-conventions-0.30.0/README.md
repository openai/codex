![OpenTelemetry — An observability framework for cloud-native software.][splash]

[splash]: https://raw.githubusercontent.com/open-telemetry/opentelemetry-rust/main/assets/logo-text.png

# OpenTelemetry Semantic Conventions

Semantic conventions for applications instrumented with [`OpenTelemetry`].

[![Crates.io: opentelemetry-semantic-conventions](https://img.shields.io/crates/v/opentelemetry-semantic-conventions.svg)](https://crates.io/crates/opentelemetry-semantic-conventions)
[![Documentation](https://docs.rs/opentelemetry-semantic-conventions/badge.svg)](https://docs.rs/opentelemetry-semantic-conventions)
[![LICENSE](https://img.shields.io/crates/l/opentelemetry-semantic-conventions)](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-semantic-conventions/LICENSE)
[![GitHub Actions CI](https://github.com/open-telemetry/opentelemetry-rust/workflows/CI/badge.svg)](https://github.com/open-telemetry/opentelemetry-rust/actions?query=workflow%3ACI+branch%3Amain)
[![Slack](https://img.shields.io/badge/slack-@cncf/otel/rust-brightgreen.svg?logo=slack)](https://cloud-native.slack.com/archives/C03GDP0H023)

## Overview

[`OpenTelemetry`] is a collection of tools, APIs, and SDKs used to instrument,
generate, collect, and export telemetry data (metrics, logs, and traces) for
analysis in order to understand your software's performance and behavior. This
crate provides standardized naming patterns for attributes, and
resources to help facilitate interoperability and compatibility with processing
and visualization tools.

[`opentelemetry`]: https://crates.io/crates/opentelemetry

*[Supported Rust Versions](#supported-rust-versions)*

## Release Notes

You can find the release notes (changelog) [here](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-semantic-conventions/CHANGELOG.md).

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