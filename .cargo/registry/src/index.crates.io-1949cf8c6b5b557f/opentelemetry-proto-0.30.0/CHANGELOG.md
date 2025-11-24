# Changelog

## vNext

## 0.30.0

Released 2025-May-23

- Update `opentelemetry` dependency version to 0.30
- Updated `opentelemetry_sdk` dependency to version 0.30.0.
- **Feature**: Added Rust code generation for profiles protos. [#2979](https://github.com/open-telemetry/opentelemetry-rust/pull/2979)
- Update `tonic` dependency version to 0.13
- Update proto definitions to v1.6.0.

## 0.29.0

Released 2025-Mar-21

- Update `opentelemetry` dependency version to 0.29
- Update `opentelemetry_sdk` dependency version to 0.29

## 0.28.0

Released 2025-Feb-10

- Update `opentelemetry` dependency version to 0.28.
- Update `opentelemetry_sdk` dependency version to 0.28.
- Bump msrv to 1.75.0.
- Update proto definitions to v1.5.0 [#2439](https://github.com/open-telemetry/opentelemetry-rust/pull/2439)
- Feature flag "populate-logs-event-name" is removed as no longer relevant.
  LogRecord's `event_name()` is now automatically populated on the newly added
  "event_name" field in LogRecord proto definition.

## 0.27.0

Released 2024-Nov-11

- Update `opentelemetry` dependency version to 0.27
- Update `opentelemetry_sdk` dependency version to 0.27

## v0.26.1

- Require tonic 0.12.3 to match generated gRPC code [#2168](https://github.com/open-telemetry/opentelemetry-rust/pull/2168)

## v0.26.0
Released 2024-Sep-30

- Update `opentelemetry` dependency version to 0.26
- Update `opentelemetry_sdk` dependency version to 0.26
- Fix JSON serialization of `metrics::Exemplar` and `trace::span::Link` [#2069](https://github.com/open-telemetry/opentelemetry-rust/pull/2069)
- Bump MSRV to 1.71.1 [2140](https://github.com/open-telemetry/opentelemetry-rust/pull/2140)

## v0.25.0
- Update `opentelemetry` dependency version to 0.25
- Update `opentelemetry_sdk` dependency version to 0.25
- Starting with this version, this crate will align with `opentelemetry` crate
  on major,minor versions.
- Update protobuf definitions to v1.3.2 [#1945](https://github.com/open-telemetry/opentelemetry-rust/pull/1945)

## v0.7.0

- Bump MSRV to 1.70 [1864](https://github.com/open-telemetry/opentelemetry-rust/pull/1874)
- Group log and Span batch by their resource and instrumentation scope before exporting [#1873](https://github.com/open-telemetry/opentelemetry-rust/pull/1873).
   - Introduced `group_logs_by_resource_and_scope()` and `group_spans_by_resource_and_scope()` methods to group logs and spans by the resource and scope respectively.
- Update `opentelemetry` dependency version to 0.24
- Update `opentelemetry_sdk` dependency version to 0.24

## v0.6.0

- Update protobuf definitions to v1.3.1 [#1721](https://github.com/open-telemetry/opentelemetry-rust/pull/1721)
- Fix the feature flag condition of `opentelemetry-proto/src/transform/logs.rs` [#1746](https://github.com/open-telemetry/opentelemetry-rust/pull/1746)
- Update `opentelemetry` dependency version to 0.23
- Update `opentelemetry_sdk` dependency version to 0.23

## v0.5.0

### Changed

- Update to tonic 0.11 and prost 0.12 [#1536](https://github.com/open-telemetry/opentelemetry-rust/pull/1536)
- **Breaking** Remove support for grpcio transport [#1534](https://github.com/open-telemetry/opentelemetry-rust/pull/1534)

### Added

- Add `schemars::JsonSchema` trait support with `with-schemars` feature [#1419](https://github.com/open-telemetry/opentelemetry-rust/pull/1419)
- Update protobuf definitions to v1.1.0 [#1482](https://github.com/open-telemetry/opentelemetry-rust/pull/1482)

## v0.4.0

### Added

- Implement tonic metrics proto transformations [#1184](https://github.com/open-telemetry/opentelemetry-rust/pull/1184)
- Move proto for zPage to tonic [#1214](https://github.com/open-telemetry/opentelemetry-rust/pull/1214)
- Support exponential histograms [#1267](https://github.com/open-telemetry/opentelemetry-rust/pull/1267)

### Changed

- Bump MSRV to 1.65 [#1318](https://github.com/open-telemetry/opentelemetry-rust/pull/1318)
- Bump MSRV to 1.64 [#1203](https://github.com/open-telemetry/opentelemetry-rust/pull/1203)

### Fixed

- Rename `traces` feature to the more standard `trace` [#1183](https://github.com/open-telemetry/opentelemetry-rust/pull/1183)

### Changed

- Switch to `prost` for `grpcio` protos. [#1202](https://github.com/open-telemetry/opentelemetry-rust/pull/1202)
  The `gen-protoc` feature is accordingly renamed to `gen-grpcio`.

## v0.3.0

### Updated

- Update protobuf definitions to v1.0.0 #1154
- Update to opentelemetry-api v0.20.0

## v0.2.0
### Changed
- Update to opentelemetry v0.19.0.
- Remove build script and generate files using unit tests [#881](https://github.com/open-telemetry/opentelemetry-rust/pull/881).
- Bump MSRV to 1.57 [#953](https://github.com/open-telemetry/opentelemetry-rust/pull/953).
- Update dependencies and bump MSRV to 1.60 [#969](https://github.com/open-telemetry/opentelemetry-rust/pull/969).
- Bump to use the v0.19.0 protobuf definitions. [#989](https://github.com/open-telemetry/opentelemetry-rust/pull/989).

## v0.1.0

Initial crate release.
