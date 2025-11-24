# Changelog

## vNext

## 0.30.0

Released 2025-May-23

- Update to [v1.32.0](https://github.com/open-telemetry/semantic-conventions/releases/tag/v1.32.0) of the semantic conventions.

## 0.29.0

Released 2025-Mar-21

- Update to [v1.31.0](https://github.com/open-telemetry/semantic-conventions/releases/tag/v1.31.0) of the semantic conventions.

## 0.28.0

Released 2025-Feb-10

- Update to [v1.29.0](https://github.com/open-telemetry/semantic-conventions/releases/tag/v1.29.0) of the semantic conventions.
- Bump msrv to 1.75.0.

## 0.27.0

Released 2024-Nov-11

- Bump MSRV to 1.70 [#2179](https://github.com/open-telemetry/opentelemetry-rust/pull/2179)
- Update to [v1.28.0](https://github.com/open-telemetry/semantic-conventions/releases/tag/v1.28.0) of the semantic conventions.

## v0.26.0
Released 2024-Sep-30

### Changed

- Starting with this version, this crate will use Weaver for the generation of
  the semantic conventions.
- **Breaking** Introduced a new feature `semconv_experimental` to enable experimental semantic conventions.
  This feature is disabled by default.

## v0.25.0
### Changed

- Starting with this version, this crate will align with `opentelemetry` crate
  on major,minor versions.
- Update to [v1.27.0](https://github.com/open-telemetry/semantic-conventions/releases/tag/v1.27.0) of the semantic conventions.
  [#2000](https://github.com/open-telemetry/opentelemetry-rust/pull/2000)

## v0.16.0
### Changed

- **Breaking** Moved duplicated (and unrelated) attributes from `opentelemetry_semantic_conventions::trace` and `opentelemetry_semantic_conventions::resource` into `opentelemetry_semantic_conventions::attribute` (which now contains all semantic attributes). `trace` and `resource` now only contain references to attributes which fall under their respective category.

### Added

- Created `opentelemetry_semantic_conventions::metric` to store metric semantic conventions.

## v0.15.0

### Changed

- Update to [v1.24.0](https://github.com/open-telemetry/semantic-conventions/releases/tag/v1.24.0) of the semantic conventions.
  [#1596](https://github.com/open-telemetry/opentelemetry-rust/pull/1596)
- Update to [v1.25.0](https://github.com/open-telemetry/semantic-conventions/releases/tag/v1.25.0) of the semantic conventions.
  [#1681](https://github.com/open-telemetry/opentelemetry-rust/pull/1681)

## v0.14.0

### Changed

- **Breaking** Replaced Key constants with &'static str for tracing compatibility
  [#1334](https://github.com/open-telemetry/opentelemetry-rust/pull/1334)

## v0.13.0

### Changed

- Bump MSRV to 1.65 [#1318](https://github.com/open-telemetry/opentelemetry-rust/pull/1318)
- Bump MSRV to 1.64 [#1203](https://github.com/open-telemetry/opentelemetry-rust/pull/1203)

## v0.12.0

### Changed

- Update to v1.21.0 spec
- Update to opentelemetry-api v0.20.0

## v0.11.0

### Changed
- Update to `opentelemetry` v0.19.
- Update to `opentelemetry_http` v0.8.
- Bump MSRV to 1.57 [#953](https://github.com/open-telemetry/opentelemetry-rust/pull/953).
- Update to v1.17.0 spec [#960](https://github.com/open-telemetry/opentelemetry-rust/pull/960).
- Update dependencies and bump MSRV to 1.60 [#969](https://github.com/open-telemetry/opentelemetry-rust/pull/969).

## v0.10.0

### Changed

- update to v1.9 spec #754
- Update to opentelemetry v0.18.0

## v0.9.0

### Changed

- Update to opentelemetry v0.17.0

## v0.8.0

### Changed

- Update to opentelemetry v0.16.0

## v0.7.0

### Changed

- Update to spec version 1.4.0 #570
- Update to opentelemetry v0.15.0

## v0.6.0

### Changed

- Update to spec version 1.3.0 #547
- Update to opentelemetry v0.14.0

## v0.5.0

### Changed
- Update to opentelemetry v0.13.0

### Removed
- Removed `from_env` and use environment variables to initialize the configurations by default #459

## v0.4.0

### Changed
- Update to opentelemetry v0.12.0

## v0.3.0

### Changed

- Update to opentelemetry v0.11.0

## v0.2.0

### Changed

- Update to opentelemetry v0.10.0

## v0.1.0

### Added

- Semantic conventions for [trace](https://github.com/open-telemetry/opentelemetry-specification/tree/master/specification/trace/semantic_conventions) and [resource](https://github.com/open-telemetry/opentelemetry-specification/tree/master/specification/resource/semantic_conventions).
