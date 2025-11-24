# Changelog

## Unreleased

The full list of changes can be found in the compare view for the respective release at <https://github.com/open-telemetry/opentelemetry-proto/releases>.

## 1.6.0 - 2025-04-29

### Added

- resource: Add EntityRef. [#635](https://github.com/open-telemetry/opentelemetry-proto/pull/635)

### Changed

- logs: Stabilize `event_name` field in `LogRecord` message. [#643](https://github.com/open-telemetry/opentelemetry-proto/pull/643)
- profiles: Move the lookup tables to ProfilesData. [#644](https://github.com/open-telemetry/opentelemetry-proto/pull/644)
- profiles: Move default sample_type from the string table to sample_type. [#620](https://github.com/open-telemetry/opentelemetry-proto/pull/620)

## 1.5.0 - 2024-12-12

### Added

- all: Add note about `schema_url` format (including version). [#605](https://github.com/open-telemetry/opentelemetry-proto/pull/605)
- logs: Add top-level `event_name` field to log records instead of `event.name` attribute. [#600](https://github.com/open-telemetry/opentelemetry-proto/pull/600)

### Removed

- profiles: Remove unused `Label` definition. [#602](https://github.com/open-telemetry/opentelemetry-proto/pull/602)
- profiles: drop duplicate `attributes` field in message Profile. [#606](https://github.com/open-telemetry/opentelemetry-proto/pull/606)

## 1.4.0 - 2024-11-20

### Added

* metrics: Add resource attributes and scope to metrics proto diagram. [#519](https://github.com/open-telemetry/opentelemetry-proto/pull/519)
* metrics: Added json example for exponential histogram. [#580](https://github.com/open-telemetry/opentelemetry-proto/pull/580)

### Changed

* metrics: Clarify aggregation temporality for Summary metric type. [#591](https://github.com/open-telemetry/opentelemetry-proto/pull/591)
* docs: Remove HTTP 1.1 restriction from Protocol Details [#571](https://github.com/open-telemetry/opentelemetry-proto/pull/571)
* docs: Update specification to include development profiles [#582](https://github.com/open-telemetry/opentelemetry-proto/pull/582)
* docs: update references to logging exporter [#581](https://github.com/open-telemetry/opentelemetry-proto/pull/581)
* Makefile: exclude Profiles protocol from breaking-changes [#576](https://github.com/open-telemetry/opentelemetry-proto/pull/576)
* Makefile: exclude Profiles service from breaking changes too [#586](https://github.com/open-telemetry/opentelemetry-proto/pull/586/)
* profiles: align type of index into string table [#557](https://github.com/open-telemetry/opentelemetry-proto/pull/557)
* profiles: drop Sample.stacktrace_id_index [#575](https://github.com/open-telemetry/opentelemetry-proto/pull/575)
* profiles: drop BuildIdKind [#584](https://github.com/open-telemetry/opentelemetry-proto/pull/584)
* profiles: drop Sample.label [#583](https://github.com/open-telemetry/opentelemetry-proto/pull/583)
* profiles: drop Location.type_index [#578](https://github.com/open-telemetry/opentelemetry-proto/pull/578)
* profiles: Rename profiles v1experimental to v1development [#585](https://github.com/open-telemetry/opentelemetry-proto/pull/585)
* profiles: Make mapping in Profile optional [#556](https://github.com/open-telemetry/opentelemetry-proto/pull/556)
* profiles: Fold the content of pprofextended.proto into profiles.proto, removing ProfileContainer. [#590](https://github.com/open-telemetry/opentelemetry-proto/pull/590)
* profiles: Improve lookup table pattern use in profiles. [#592](https://github.com/open-telemetry/opentelemetry-proto/pull/592)
* profiles: Renovations to experimental profiling schema. [#596](https://github.com/open-telemetry/opentelemetry-proto/pull/596)

## 1.3.2 - 2024-06-28

### Changed

* profiles: add missing java_package option to pprofextended. [#558](https://github.com/open-telemetry/opentelemetry-proto/pull/558)

## 1.3.1 - 2024-05-07

### Changed

* profiles: fix versioning in selector. [#551](https://github.com/open-telemetry/opentelemetry-proto/pull/551)

## 1.3.0 - 2024-04-24

### Added

* Add new profile signal.
  [#534](https://github.com/open-telemetry/opentelemetry-proto/pull/534)

## 1.2.0 - 2024-03-29

### Added

* Indicate if a `Span`'s parent or link is remote using 2 bit flag.
  [#484](https://github.com/open-telemetry/opentelemetry-proto/pull/484)
* Add metric.metadata for supporting additional metadata on metrics
  [#514](https://github.com/open-telemetry/opentelemetry-proto/pull/514)
* Add example of an Event [#538](https://github.com/open-telemetry/opentelemetry-proto/pull/538)

### Changed

## 1.1.0 - 2024-01-10

Full list of differences found in [this compare](https://github.com/open-telemetry/opentelemetry-proto/compare/v1.0.0...v1.1.0).

### Added

* Add `flags` field to `Span` and `Span/Link` for W3C-specified Trace Context flags.
  [#503](https://github.com/open-telemetry/opentelemetry-proto/pull/503)

### Changed

* Update and fix OTLP JSON examples. [#516](https://github.com/open-telemetry/opentelemetry-proto/pull/516),
  [#510](https://github.com/open-telemetry/opentelemetry-proto/pull/510),
  [#499](https://github.com/open-telemetry/opentelemetry-proto/pull/499)
* Remove irrelevant comments from metric name field. [#512](https://github.com/open-telemetry/opentelemetry-proto/pull/512)
* Add comment to explain schema_url fields. [#504](https://github.com/open-telemetry/opentelemetry-proto/pull/504)

## 1.0.0 - 2023-07-03

Full list of differences found in [this compare](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.20.0...v1.0.0).

### Maturity

* Add note about the possibility to have unstable components after 1.0.0
  [#489](https://github.com/open-telemetry/opentelemetry-proto/pull/489)
* Add maturity JSON entry per package
  [#490](https://github.com/open-telemetry/opentelemetry-proto/pull/490)

## 0.20.0 - 2023-06-06

Full list of differences found in [this compare](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.19.0...v0.20.0).

### Maturity

* Declare OTLP/JSON Stable.
  [#436](https://github.com/open-telemetry/opentelemetry-proto/pull/436)
  [#435](https://github.com/open-telemetry/opentelemetry-proto/pull/435)
* Provide stronger symbolic stability guarantees.
  [#432](https://github.com/open-telemetry/opentelemetry-proto/pull/432)
* Clarify how additive changes are handled.
  [#455](https://github.com/open-telemetry/opentelemetry-proto/pull/455)

### Changed

* Change the exponential histogram boundary condition.
  [#409](https://github.com/open-telemetry/opentelemetry-proto/pull/409)
* Clarify behavior for empty/not present/invalid trace_id and span_id fields.
  [#442](https://github.com/open-telemetry/opentelemetry-proto/pull/442)
* Change the collector trace endpoint to /v1/traces.
  [#449](https://github.com/open-telemetry/opentelemetry-proto/pull/449)

### Added

* Introduce `zero_threshold` field to `ExponentialHistogramDataPoint`.
  [#441](https://github.com/open-telemetry/opentelemetry-proto/pull/441)
  [#453](https://github.com/open-telemetry/opentelemetry-proto/pull/453)

### Removed

* Delete requirement to generate new trace/span id if an invalid id is received.
  [#444](https://github.com/open-telemetry/opentelemetry-proto/pull/444)

## 0.19.0 - 2022-08-03

Full list of differences found in [this compare](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.18.0...v0.19.0).

### Changed

* Add `csharp_namespace` option to protos.
  ([#399](https://github.com/open-telemetry/opentelemetry-proto/pull/399))
* Fix some out-of-date urls which link to [specification](https://github.com/open-telemetry/opentelemetry-specification). ([#402](https://github.com/open-telemetry/opentelemetry-proto/pull/402))
* :stop_sign: [BREAKING] Delete deprecated InstrumentationLibrary,
  InstrumentationLibraryLogs, InstrumentationLibrarySpans and
  InstrumentationLibraryMetrics messages. Delete deprecated
  instrumentation_library_logs, instrumentation_library_spans and
  instrumentation_library_metrics fields.

### Added

* Introduce Scope Attributes. [#395](https://github.com/open-telemetry/opentelemetry-proto/pull/395)
* Introduce partial success fields in `Export<signal>ServiceResponse`.
 [#414](https://github.com/open-telemetry/opentelemetry-proto/pull/414)

## 0.18.0 - 2022-05-17

Full list of differences found in [this compare](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.17.0...v0.18.0).

### Changed

* Declare logs Stable.
  [(#376)](https://github.com/open-telemetry/opentelemetry-proto/pull/376)
* Metrics ExponentialHistogramDataPoint makes the `sum` optional
  (follows the same change in HistogramDataPOint in 0.15). [#392](https://github.com/open-telemetry/opentelemetry-proto/pull/392)

## 0.17.0 - 2022-05-06

Full list of differences found in [this compare](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.16.0...v0.17.0).

### Changed

* Introduce optional `min` and `max` fields to the Histogram and ExponentialHistogram data points.
  [(#279)](https://github.com/open-telemetry/opentelemetry-proto/pull/279)

## 0.16.0 - 2022-03-31

Full list of differences found in [this compare](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.15.0...v0.16.0).

### Removed

* Remove deprecated LogRecord.Name field (#373).

## 0.15.0 - 2022-03-19

Full list of differences found in [this compare](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.14.0...v0.15.0).

### Changed

* Rename InstrumentationLibrary to InstrumentationScope (#362)

### Added

* Use optional for `sum` field to mark presence (#366)

## 0.14.0 - 2022-03-08

Full list of differences found in [this compare](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.13.0...v0.14.0).

### Removed

* Deprecate LogRecord.Name field (#357)

## 0.13.0 - 2022-02-10

Full list of differences found in [this compare](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.12.0...v0.13.0).

### Changed

* `Swagger` generation updated to `openapiv2` due to the use of an updated version of protoc in `otel/build-protobuf`
* Clarify attribute key uniqueness requirement (#350)
* Fix path to Go packages (#360)

### Added

* Add ObservedTimestamp to LogRecord. (#351)
* Add native kotlin support (#337)

### Removed

* Remove unused deprecated message StringKeyValue (#358)
* Remove experimental metrics config service (#359)

## 0.12.0 - 2022-01-19

Full list of differences found in [this compare](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.11.0...v0.12.0).

### Changed

* Rename logs to log_records in InstrumentationLibraryLogs. (#352)

### Removed

* Remove deprecated messages and fields from traces. (#341)
* Remove deprecated messages and fields from metrics. (#342)

## 0.11.0 - 2021-10-07

Full list of differences found in [this compare](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.10.0...v0.11.0).

### Added

* ExponentialHistogram is a base-2 exponential histogram described in [OTEP 149](https://github.com/open-telemetry/oteps/pull/149). (#322)
* Adds `TracesData`, `MetricsData`, and `LogsData` container types for common use
  in transporting multiple `ResourceSpans`, `ResourceMetrics`, and `ResourceLogs`. (#332)

## 0.10.0 - 2021-09-07

Full list of differences found in [this compare.](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.9.0...v0.10.0)

### Maturity

* `collector/logs/*` is now considered `Beta`. (#311)
* `logs/*` is now considered `Beta`. (#311)

### Added

* Metrics data points add a `flags` field with one bit to represent explicitly missing data. (#316)

## 0.9.0 - 2021-04-12

Full list of differences found in [this compare.](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.8.0...v0.9.0)

### Maturity

* `collector/metrics/*` is now considered `stable`. (#305)

### Changed: Metrics

* :stop_sign: [DATA MODEL CHANGE] Histogram/Summary sums must be monotonic counters of events (#302)
* :stop_sign: [DATA MODEL CHANGE] Clarify requirements and semantics for start time (#295)
* :stop_sign: [BREAKING] Deprecate `labels` field from NumberDataPoint, HistogramDataPoint, SummaryDataPoint and add equivalent `attributes` field (#283)
* :stop_sign: [BREAKING] Deprecate `filtered_labels` field from Exemplars and add equivalent `filtered_attributes` field (#283)

### Added

- Common - Add bytes (binary) as data type to AnyValue (#297)
- Common - Add schema_url fields as described in OTEP 0152 (#298)

### Removed

* Remove if no changes for this section before release.

## 0.8.0 - 2021-03-23

Full list of differences found in [this compare.](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.7.0...v0.8.0)

### Historical breaking change notice

Release 0.8 was the last in the line of releases marked as "unstable".
This release broke compatibility in more ways than were recognized and
documented at the time of its release.  In particular, #278 created
the `NumberDataPoint` type and used it in several locations in place
of the former `DoubleDataPoint`.  The new `oneof` in `NumberDataPoint`
re-used the former `DoubleDataPoint` tag number, which means that
pre-0.8 `DoubleSum` and `DoubleGauge` points would parse correctly as
a 0.8 `Sum` and `Gauge` points containing double-valued numbers.

However, by virtue of a `syntax = "proto3"` declaration, the protocol
compiler for all versions of OTLP have not included field presence,
which means 0 values are not serialized.  **The result is that valid
OTLP 0.7 `DoubleSum` and `DoubleGauge` points would not parse
correctly as OTLP 0.8 data.**  Instead, they parse as
`NumberDataPoint` with a missing value in the `oneof` field.

### Changed: Metrics

* :stop_sign: [DEPRECATION] Deprecate IntSum, IntGauge, and IntDataPoint (#278)
* :stop_sign: [DEPRECATION] Deprecate IntExemplar (#281)
* :stop_sign: [DEPRECATION] Deprecate IntHistogram (#270)
* :stop_sign: [BREAKING] Rename DoubleGauge to Gauge (#278)
* :stop_sign: [BREAKING] Rename DoubleSum to Sum (#278)
* :stop_sign: [BREAKING] Rename DoubleDataPoint to NumberDataPoint (#278)
* :stop_sign: [BREAKING] Rename DoubleSummary to Summary (#269)
* :stop_sign: [BREAKING] Rename DoubleExemplar to Exemplar (#281)
* :stop_sign: [BREAKING] Rename DoubleHistogram to Histogram (#270)
* :stop_sign: [DATA MODEL CHANGE] Make explicit bounds compatible with OM/Prometheus (#262)

## 0.7.0 - 2021-01-28

Full list of differences found in [this compare.](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.6.0...v0.7.0)

### Maturity

$$$Protobuf Encodings:**

* `collector/metrics/*` is now considered `Beta`. (#223)
* `collector/logs/*` is now considered `Alpha`. (#228)
* `logs/*` is now considered `Alpha`. (#228)
* `metrics/*` is now considered `Beta`. (#223)

### Changed

* Common/Logs/Metrics/Traces - Clarify empty instrumentation (#245)

### Added

* Metrics - Add SummaryDataPoint support to Metrics proto (#227)

## 0.6.0 - 2020-10-28

Full list of differences found in [this compare.](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.5.0...v0.6.0)

### Maturity

* Clarify maturity guarantees (#225)

### Changed

* Traces - Deprecated old Span status code and added a new status code according to specification (#224)
** Marked for removal `2021-10-22` given Stability Guarantees.
* Rename ProbabilitySampler to TraceIdRatioBased (#221)

## 0.5.0 - 2020-08-31

Full list of differences found in [this compare.](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.4.0...v0.5.0)

### Maturity Changes

**Protobuf Encodings:**

* `collector/trace/*` is now `Stable`.
* `common/*` is now `Stable`.
* `resource/*` is now `Stable`.
* `trace/trace.proto` is now `Stable`. (#160)

**JSON Encodings:**

* All messages are now `Alpha`.

### Changed

* :stop_sign: [BREAKING] Metrics - protocol was refactored, and lots of breaking changes.
  * Removed MetricDescriptor and embedded into Metric and the new data types.
  * Add new data types Gauge/Sum/Histogram.
  * Make use of the "AggregationTemporality" into the data types that allow that support.
* Rename enum values to follow the proto3 style guide.

### Added

* Enable build to use docker image otel/build-protobuf to be used in CI.
** Can also be used by the languages to generate protos.

### Removed

* :stop_sign: [BREAKING] Remove generated golang structs from the repository

### Errata

The following was announced in the release, but has not yet been considered stable. Please see the latest
README.md for actual status.

> This is a Release Candidate to declare Metrics part of the protocol Stable.

## 0.4.0 - 2020-06-23

Full list of differences found in [this compare.](https://github.com/open-telemetry/opentelemetry-proto/compare/v0.3.0...v0.4.0)

### Changed

* Metrics - Add temporality to MetricDescriptor (#140).

### Added

* Metrics - Add Monotonic Types (#145)
* Common/Traces - Added support for arrays and maps for attribute values (AnyValue) (#157).

### Removed

* :stop_sign: [BREAKING] Metrics - Removed common labels from MetricDescriptor (#144).

### Errata

The following was announced in the release, but this was not considered Stable until `v0.5.0`

> This is a Release Candidate to declare Traces part of the protocol Stable.

## 0.3.0 - 2020-03-23

* Initial protos for trace, metrics, resource and OTLP.
