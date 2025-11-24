# Changelog

## vNext

## 0.30.0

Released 2025-May-23

- Updated `opentelemetry` dependency to version 0.30.0.

## 0.29.0

Released 2025-Mar-21

- Update `opentelemetry` dependency version to 0.29.

## 0.28.0

Released 2025-Feb-10

- Update `opentelemetry` dependency version to 0.28.
- Bump msrv to 1.75.0.
- Add "internal-logs" feature flag (enabled by default), and emit internal logs via `tracing` crate.
- Add `HttpClient::send_bytes` with `bytes::Bytes` request payload and deprecate old `HttpClient::send` function.

## 0.27.0

Released 2024-Nov-08

- Update `opentelemetry` dependency version to 0.27

- Bump MSRV to 1.70 [#2179](https://github.com/open-telemetry/opentelemetry-rust/pull/2179)

## v0.26.0
Released 2024-Sep-30

- Update `opentelemetry` dependency version to 0.26

## v0.25.0

- Update `opentelemetry` dependency version to 0.25
- Starting with this version, this crate will align with `opentelemetry` crate
  on major,minor versions.
  
## v0.13.0

- **Breaking** Correct the misspelling of "webkpi" to "webpki" in features [#1842](https://github.com/open-telemetry/opentelemetry-rust/pull/1842)
- **Breaking** Remove support for the `isahc` HTTP client [#1924](https://github.com/open-telemetry/opentelemetry-rust/pull/1924)
- Update to `http` v1 [#1674](https://github.com/open-telemetry/opentelemetry-rust/pull/1674)
- Update `opentelemetry` dependency version to 0.24

## v0.12.0

- Add `reqwest-rustls-webkpi-roots` feature flag to configure [`reqwest`](https://docs.rs/reqwest/0.11.27/reqwest/index.html#optional-features) to use embedded `webkpi-roots`.
- Update `opentelemetry` dependency version to 0.23

## v0.11.1

- Add feature flag enabling users to configure `reqwest` usage to use rustls via
  `reqwest/rustls-tls` feature flag
  [1638](https://github.com/open-telemetry/opentelemetry-rust/pull/1638).

## v0.11.0

### Changed

- **Breaking** Remove built-in support for surf HTTP client [#1537](https://github.com/open-telemetry/opentelemetry-rust/pull/1537)
- **Breaking** Surface non-2xx status codes as errors; change `ResponseExt` trait to return `HttpError` instead of `TraceError`[#1484](https://github.com/open-telemetry/opentelemetry-rust/pull/1484)

## v0.10.0

### Changed

- Bump MSRV to 1.65 [#1318](https://github.com/open-telemetry/opentelemetry-rust/pull/1318)
- Bump MSRV to 1.64 [#1203](https://github.com/open-telemetry/opentelemetry-rust/pull/1203)

## v0.9.0

### Changed

- Update to opentelemetry-api v0.20.0

## v0.8.0

### Changed
- Add response headers in response for `HttpClient` implementations [#918](https://github.com/open-telemetry/opentelemetry-rust/pull/918).
- Bump MSRV to 1.57 [#953](https://github.com/open-telemetry/opentelemetry-rust/pull/953).
- Update dependencies and bump MSRV to 1.60 [#969](https://github.com/open-telemetry/opentelemetry-rust/pull/969).

## v0.7.0

### Changed

- Update to opentelemetry v0.18.0
- Export `byte` and `http` types #798
- Implementation of collector http client with pure hyper #853

## v0.6.0

### Changed

- Update to opentelemetry v0.17.0

## v0.5.0

### Changed

- Update to opentelemetry v0.16.0

## v0.4.0

### Changed

- Update to opentelemetry v0.15.0

## v0.3.0

### Changed

- Return response from `HttpClient` #511
- Update to opentelemetry v0.14.0

## v0.2.0

### Changed
- Update to opentelemetry v0.13.0

## v0.1.0

### Added

- Opentelemetry integration with http #415
