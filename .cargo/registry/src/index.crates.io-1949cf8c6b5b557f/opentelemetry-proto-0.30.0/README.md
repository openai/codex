![OpenTelemetry — An observability framework for cloud-native software.][splash]

[splash]: https://raw.githubusercontent.com/open-telemetry/opentelemetry-rust/main/assets/logo-text.png

# OpenTelemetry Proto
This crate contains generated files from [opentelemetry-proto](https://github.com/open-telemetry/opentelemetry-proto)
repository and transformation between types from generated files and types defined in [opentelemetry](https://github.com/open-telemetry/opentelemetry-rust/tree/main/opentelemetry).

*[Supported Rust Versions](#supported-rust-versions)*

## Release Notes

You can find the release notes (changelog) [here](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-proto/CHANGELOG.md).

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