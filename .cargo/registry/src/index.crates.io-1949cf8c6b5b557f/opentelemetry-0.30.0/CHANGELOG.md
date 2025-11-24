# Changelog

## vNext

## 0.30.0

Released 2025-May-23

[#2821](https://github.com/open-telemetry/opentelemetry-rust/pull/2821) Context
based suppression capabilities added: Added the ability to prevent recursive
telemetry generation through new context-based suppression mechanisms. This
feature helps prevent feedback loops and excessive telemetry when OpenTelemetry
components perform their own operations.

New methods added to `Context`:

- `is_telemetry_suppressed()` - Checks if telemetry is suppressed in this
  context
- `with_telemetry_suppressed()` - Creates a new context with telemetry
  suppression enabled
- `is_current_telemetry_suppressed()` - Efficiently checks if the current thread's context
  has telemetry suppressed
- `enter_telemetry_suppressed_scope()` - Convenience method to enter a scope where telemetry is
  suppressed

These methods allow SDK components, exporters, and processors to temporarily
disable telemetry generation during their internal operations, ensuring more
predictable and efficient observability pipelines.

- re-export `tracing` for `internal-logs` feature to remove the need of adding `tracing` as a dependency

## 0.29.1

Release 2025-Apr-01

- Bug Fix: Re-export `WithContext` at `opentelemetry::trace::context::WithContext` [#2879](https://github.com/open-telemetry/opentelemetry-rust/pull/2879) to restore backwards compatability
  - The new path for `WithContext` and `FutureExt` are in  `opentelemetry::context` as they are independent of the trace signal. Users should prefer this path.

## 0.29.0

Released 2025-Mar-21

- *Breaking* Moved `ExportError` trait from `opentelemetry::trace::ExportError` to `opentelemetry_sdk::export::ExportError`
- *Breaking* Moved `TraceError` enum from `opentelemetry::trace::TraceError` to `opentelemetry_sdk::trace::TraceError`
- *Breaking* Moved `TraceResult` type alias from `opentelemetry::trace::TraceResult` to `opentelemetry_sdk::trace::TraceResult`
- Bug Fix: `InstrumentationScope` implementation for `PartialEq` and `Hash` fixed to include Attributes also.
- **Breaking changes for baggage users**: [#2717](https://github.com/open-telemetry/opentelemetry-rust/issues/2717)
  - Changed value type of `Baggage` from `Value` to `StringValue`
  - Updated `Baggage` constants to reflect latest standard (`MAX_KEY_VALUE_PAIRS` - 180 -> 64, `MAX_BYTES_FOR_ONE_PAIR` - removed) and increased insert performance see #[2284](https://github.com/open-telemetry/opentelemetry-rust/pull/2284).
  - Align `Baggage.remove()` signature with `.get()` to take the key as a reference
  - `Baggage` can't be retrieved from the `Context` directly anymore and needs to be accessed via `context.baggage()`
  - `with_baggage()` and `current_with_baggage()` override any existing `Baggage` in the `Context`
  - `Baggage` keys can't be empty and only allow ASCII visual chars, except `"(),/:;<=>?@[\]{}` (see [RFC7230, Section 3.2.6](https://datatracker.ietf.org/doc/html/rfc7230#section-3.2.6))
  - `KeyValueMetadata` does not publicly expose its fields. This should be transparent change to the users.
- Changed `Context` to use a stack to properly handle out of order dropping of `ContextGuard`. This imposes a limit of `65535` nested contexts on a single thread. See #[2378](https://github.com/open-telemetry/opentelemetry-rust/pull/2284) and #[1887](https://github.com/open-telemetry/opentelemetry-rust/issues/1887).
- Added additional `name: Option<&str>` parameter to the `event_enabled` method
  on the `Logger` trait. This allows implementations (SDK, processor, exporters)
  to leverage this additional information to determine if an event is enabled.

## 0.28.0

Released 2025-Feb-10

- Bump msrv to 1.75.0.
- **Breaking** `opentelemetry::global::shutdown_tracer_provider()` Removed from this crate, should now use `tracer_provider.shutdown()` see [#2369](https://github.com/open-telemetry/opentelemetry-rust/pull/2369) for a migration example.
- *Breaking* Removed unused `opentelemetry::PropagationError` struct.

## 0.27.1

Released 2024-Nov-27

## 0.27.0

Released 2024-Nov-11

- Bump MSRV to 1.70 [#2179](https://github.com/open-telemetry/opentelemetry-rust/pull/2179)
- Add `LogRecord::set_trace_context`; an optional method conditional on the `trace` feature for setting trace context on a log record.
- Removed unnecessary public methods named `as_any` from `AsyncInstrument` trait and the implementing instruments: `ObservableCounter`, `ObservableGauge`, and `ObservableUpDownCounter` [#2187](https://github.com/open-telemetry/opentelemetry-rust/pull/2187)
- Introduced `SyncInstrument` trait to replace the individual synchronous instrument traits (`SyncCounter`, `SyncGauge`, `SyncHistogram`, `SyncUpDownCounter`) which are meant for SDK implementation. [#2207](https://github.com/open-telemetry/opentelemetry-rust/pull/2207)
- Ensured that `observe` method on asynchronous instruments can only be called inside a callback. This was done by removing the implementation of `AsyncInstrument` trait for each of the asynchronous instruments. [#2210](https://github.com/open-telemetry/opentelemetry-rust/pull/2210)
- Removed `PartialOrd` and `Ord` implementations for `KeyValue`. [#2215](https://github.com/open-telemetry/opentelemetry-rust/pull/2215)
- **Breaking change for exporter authors:** Marked `KeyValue` related structs and enums as `non_exhaustive`. [#2228](https://github.com/open-telemetry/opentelemetry-rust/pull/2228)
- **Breaking change for log exporter authors:** Marked `AnyValue` enum as `non_exhaustive`. [#2230](https://github.com/open-telemetry/opentelemetry-rust/pull/2230)
- **Breaking change for Metrics users:** The `init` method used to create instruments has been renamed to `build`. Also, `try_init()` method is removed from instrument builders. The return types of `InstrumentProvider` trait methods modified to return the instrument struct, instead of `Result`. [#2227](https://github.com/open-telemetry/opentelemetry-rust/pull/2227)

Before:
```rust
let counter = meter.u64_counter("my_counter").init();
```

Now:
```rust
let counter = meter.u64_counter("my_counter").build();
```
- **Breaking change**: [#2220](https://github.com/open-telemetry/opentelemetry-rust/pull/2220)
  - Removed deprecated method `InstrumentationLibrary::new`
  - Renamed `InstrumentationLibrary` to `InstrumentationScope`
  - Renamed `InstrumentationLibraryBuilder` to `InstrumentationScopeBuilder`
  - Removed deprecated methods `LoggerProvider::versioned_logger` and `TracerProvider::versioned_tracer`
  - Removed methods `LoggerProvider::logger_builder`, `TracerProvider::tracer_builder` and `MeterProvider::versioned_meter`
  - Replaced these methods with `LoggerProvider::logger_with_scope`, `TracerProvider::logger_with_scope`, `MeterProvider::meter_with_scope`
  - Replaced `global::meter_with_version` with `global::meter_with_scope`
  - Added `global::tracer_with_scope`
  - Refer to PR description for migration guide.
- **Breaking change**: replaced `InstrumentationScope` public attributes by getters [#2275](https://github.com/open-telemetry/opentelemetry-rust/pull/2275)  

- **Breaking change**: [#2260](https://github.com/open-telemetry/opentelemetry-rust/pull/2260)
  - Removed `global::set_error_handler` and `global::handle_error`. 
  - `global::handle_error` usage inside the opentelemetry crates has been replaced with `global::otel_info`, `otel_warn`, `otel_debug` and `otel_error` macros based on the severity of the internal logs.
  - The default behavior of `global::handle_error` was to log the error using `eprintln!`. With otel macros, the internal logs get emitted via `tracing` macros of matching severity. Users now need to configure a `tracing` layer/subscriber to capture these logs.
  - Refer to PR description for migration guide. Also refer to [self-diagnostics](https://github.com/open-telemetry/opentelemetry-rust/tree/main/examples/self-diagnostics) example to learn how to view internal logs in stdout using `tracing::fmt` layer.

- **Breaking change for exporter/processor authors:** [#2266](https://github.com/open-telemetry/opentelemetry-rust/pull/2266)
   - Moved `ExportError` trait from `opentelemetry::ExportError` to `opentelemetry_sdk::export::ExportError`
   - Created new trait `opentelemetry::trace::ExportError` for trace API. This would be eventually be consolidated with ExportError in the SDK.
   - Moved `LogError` enum from `opentelemetry::logs::LogError` to `opentelemetry_sdk::logs::LogError`
   - Moved `LogResult` type alias from `opentelemetry::logs::LogResult` to `opentelemetry_sdk::logs::LogResult`
   - Moved `MetricError` enum from `opentelemetry::metrics::MetricError` to `opentelemetry_sdk::metrics::MetricError`
   - Moved `MetricResult` type alias from `opentelemetry::metrics::MetricResult` to `opentelemetry_sdk::metrics::MetricResult`
     These changes shouldn't directly affect the users of OpenTelemetry crate, as these constructs are used in SDK and Exporters. If you are an author of an sdk component/plug-in, like an exporter etc. please use these types from sdk. Refer [CHANGELOG.md](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-sdk/CHANGELOG.md) for more details, under same version section.
- **Breaking** [2291](https://github.com/open-telemetry/opentelemetry-rust/pull/2291) Rename `logs_level_enabled flag` to `spec_unstable_logs_enabled`. Please enable this updated flag if the feature is needed. This flag will be removed once the feature is stabilized in the specifications.

## v0.26.0
Released 2024-Sep-30

- **BREAKING** Public API changes:
  - **Removed**: `Key.bool()`, `Key.i64()`, `Key.f64()`, `Key.string()`, `Key.array()` [#2090](https://github.com/open-telemetry/opentelemetry-rust/issues/2090).     These APIs were redundant as they didn't offer any additional functionality. The existing `KeyValue::new()` API covers all the scenarios offered by these APIs.

  - **Removed**: `ObjectSafeMeterProvider` and `GlobalMeterProvider` [#2112](https://github.com/open-telemetry/opentelemetry-rust/pull/2112). These APIs were unnecessary and were mainly meant for internal use.

  - **Modified**: `MeterProvider.meter()` and `MeterProvider.versioned_meter()` argument types have been updated to `&'static str` instead of `impl Into<Cow<'static, str>>>` [#2112](https://github.com/open-telemetry/opentelemetry-rust/pull/2112). These APIs were modified to enforce the Meter `name`, `version`, and `schema_url` to be `&'static str`.

  - **Renamed**: `NoopMeterCore` to `NoopMeter`

- Added `with_boundaries` API to allow users to provide custom bounds for Histogram instruments. [#2135](https://github.com/open-telemetry/opentelemetry-rust/pull/2135)

## v0.25.0

- **BREAKING** [#1993](https://github.com/open-telemetry/opentelemetry-rust/pull/1993) Box complex types in AnyValue enum
Before:
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AnyValue {
    /// An integer value
    Int(i64),
    /// A double value
    Double(f64),
    /// A string value
    String(StringValue),
    /// A boolean value
    Boolean(bool),
    /// A byte array
    Bytes(Vec<u8>),
    /// An array of `Any` values
    ListAny(Vec<AnyValue>),
    /// A map of string keys to `Any` values, arbitrarily nested.
    Map(HashMap<Key, AnyValue>),
}
```

After:
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AnyValue {
    /// An integer value
    Int(i64),
    /// A double value
    Double(f64),
    /// A string value
    String(StringValue),
    /// A boolean value
    Boolean(bool),
    /// A byte array
    Bytes(Box<Vec<u8>>),
    /// An array of `Any` values
    ListAny(Box<Vec<AnyValue>>),
    /// A map of string keys to `Any` values, arbitrarily nested.
    Map(Box<HashMap<Key, AnyValue>>),
}
```
So the custom log appenders should box these types while adding them in message body, or
attribute values. Similarly, the custom exporters should dereference these complex type values
before serializing.

*Breaking* :
[#2015](https://github.com/open-telemetry/opentelemetry-rust/pull/2015) Removed
the ability to register callbacks for Observable instruments on Meter directly.
If you were using `meter.register_callback` to provide the callback, provide
them using `with_callback` method, while creating the Observable instrument
itself.
[1715](https://github.com/open-telemetry/opentelemetry-rust/pull/1715/files)
shows the exact changes needed to make this migration. If you are starting new,
refer to the
[examples](https://github.com/open-telemetry/opentelemetry-rust/blob/main/examples/metrics-basic/src/main.rs)
to learn how to provide Observable callbacks.

## v0.24.0

- Add "metrics", "logs" to default features. With this, default feature list is
  "trace", "metrics" and "logs".
- When "metrics" feature is enabled, `KeyValue` implements `PartialEq`, `Eq`,
  `PartialOrder`, `Order`, `Hash`. This is meant to be used for metrics
  aggregation purposes only.
- Removed `Unit` struct for specifying Instrument units. Unit is treated as an
  opaque string. Migration: Replace `.with_unit(Unit::new("myunit"))` with
  `.with_unit("myunit")`.

- [1869](https://github.com/open-telemetry/opentelemetry-rust/pull/1869) Introduced the `LogRecord::set_target()` method in the log bridge API. 
This method allows appenders to set the target/component emitting the logs.

## v0.23.0

### Added

- [#1640](https://github.com/open-telemetry/opentelemetry-rust/pull/1640) Add `PropagationError`
- [#1701](https://github.com/open-telemetry/opentelemetry-rust/pull/1701) `Gauge` no longer requires `otel-unstable` feature flag, as OpenTelemetry specification for `Gauge` instrument is stable.

### Removed

- Remove `urlencoding` crate dependency. [#1613](https://github.com/open-telemetry/opentelemetry-rust/pull/1613)
- Remove global providers for Logs [$1691](https://github.com/open-telemetry/opentelemetry-rust/pull/1691)
    LoggerProviders are not meant for end users to get loggers from. It is only required for the log bridges.
    Below global constructs for the logs are removed from API:
        - opentelemetry::global::logger
        - opentelemetry::global::set_logger_provider
        - opentelemetry::global::shutdown_logger_provider
        - opentelemetry::global::logger_provider
        - opentelemetry::global::GlobalLoggerProvider
        - opentelemetry::global::ObjectSafeLoggerProvider 
    For creating appenders using Logging bridge API, refer to the opentelemetry-tracing-appender [example](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-appender-tracing/examples/basic.rs)

### Changed

- **BREAKING** Moving LogRecord implementation to the SDK. [1702](https://github.com/open-telemetry/opentelemetry-rust/pull/1702).
    - Relocated `LogRecord` struct to SDK.
    - Introduced the `LogRecord` trait in the API for populating log records. This trait is implemented by the SDK.
    This is the breaking change for the authors of Log Appenders. Refer to the [opentelemetry-appender-tracing](https://github.com/open-telemetry/opentelemetry-rust/tree/main/opentelemetry-appender-tracing) for more details.

- Deprecate `versioned_logger()` in favor of `logger_builder()` [1567](https://github.com/open-telemetry/opentelemetry-rust/pull/1567).

Before:

```rust
let logger = provider.versioned_logger(
    "my-logger-name",
    Some(env!("CARGO_PKG_VERSION")),
    Some("https://opentelemetry.io/schema/1.0.0"),
    Some(vec![KeyValue::new("key", "value")]),
);
```

After:

```rust
let logger = provider
    .logger_builder("my-logger-name")
    .with_version(env!("CARGO_PKG_VERSION"))
    .with_schema_url("https://opentelemetry.io/schema/1.0.0")
    .with_attributes(vec![KeyValue::new("key", "value")])
    .build();
```

- Deprecate `versioned_tracer()` in favor of `tracer_builder()` [1567](https://github.com/open-telemetry/opentelemetry-rust/pull/1567).

Before:

```rust
let tracer = provider.versioned_tracer(
    "my-tracer-name",
    Some(env!("CARGO_PKG_VERSION")),
    Some("https://opentelemetry.io/schema/1.0.0"),
    Some(vec![KeyValue::new("key", "value")]),
);
```

After:

```rust
let tracer = provider
    .tracer_builder("my-tracer-name")
    .with_version(env!("CARGO_PKG_VERSION"))
    .with_schema_url("https://opentelemetry.io/schema/1.0.0")
    .with_attributes(vec![KeyValue::new("key", "value")])
    .build();
```

## v0.22.0

### Added

- [#1410](https://github.com/open-telemetry/opentelemetry-rust/pull/1410) Add experimental synchronous gauge. This is behind the feature flag, and can be enabled by enabling the feature `otel_unstable` for opentelemetry crate.

- [#1410](https://github.com/open-telemetry/opentelemetry-rust/pull/1410) Guidelines to add new unstable/experimental features.

### Changed

- Modified `AnyValue.Map` to be backed by `HashMap` instead of custom `OrderMap`,
which internally used `IndexMap`. There was no requirement to maintain the order
of entries, so moving from `IndexMap` to `HashMap` offers slight performance
gains, and avoids `IndexMap` dependency. This affects `body` and `attributes` of
`LogRecord`.
[#1353](https://github.com/open-telemetry/opentelemetry-rust/pull/1353)
- Add `TextMapCompositePropagator` [#1373](https://github.com/open-telemetry/opentelemetry-rust/pull/1373)
- Turned off events for `NoopLogger` to save on operations
  [#1455](https://github.com/open-telemetry/opentelemetry-rust/pull/1455)

### Removed

- Removed `OrderMap` type as there was no requirement to use this over regular
`HashMap`.
[#1353](https://github.com/open-telemetry/opentelemetry-rust/pull/1353)
- Remove API for Creating Histograms with signed integers. [#1371](https://github.com/open-telemetry/opentelemetry-rust/pull/1371)
- Remove `global::shutdown_meter_provider`, use `SdkMeterProvider::shutdown`
  directly instead [#1412](https://github.com/open-telemetry/opentelemetry-rust/pull/1412).

## [v0.21.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.20.0...v0.21.0)

This release should been seen as 1.0-rc4 following 1.0-rc3 in v0.20.0. Refer to CHANGELOG.md in individual creates for details on changes made in different creates.

### Changed

- Bump MSRV to 1.65 [#1318](https://github.com/open-telemetry/opentelemetry-rust/pull/1318)
- Bump MSRV to 1.64 [#1203](https://github.com/open-telemetry/opentelemetry-rust/pull/1203)
- `opentelemetry` crate now only carries the API types [#1186](https://github.com/open-telemetry/opentelemetry-rust/issues/1186). Use the `opentelemetry_sdk` crate for the SDK types.
- `trace::noop::NoopSpan` no longer implements `Default` and instead exposes
  a `const DEFAULT` value. [#1270](https://github.com/open-telemetry/opentelemetry-rust/pull/1270)
- Updated crate documentation and examples.
  [#1256](https://github.com/open-telemetry/opentelemetry-rust/issues/1256)
- **Breaking** `SpanBuilder` attributes changed from `OrderMap<Key, Value>` to
  `Vec<KeyValue>` and `with_attributes_map` method is removed from `SpanBuilder`.
  This implies that OpenTelemetry API will no longer perform
  de-dup of attribute Keys.
  [#1293](https://github.com/open-telemetry/opentelemetry-rust/issues/1293).
  Please share [feedback
  here](https://github.com/open-telemetry/opentelemetry-rust/issues/1300), if
  you are affected.

## [v0.20.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.19.0...v0.20.0)
This release should been seen as 1.0-rc3 following 1.0-rc2 in v0.19.0. Refer to CHANGELOG.md in individual creates for details on changes made in different creates.

### Added

- Add `new` method to `BoxedTracer` #1009
- Add js-sys as dependency for api crate when building wasm targets #1078
- Create tracer using a shared instrumentation library #1129
- Add `Context::map_current` #1140
- Add unit/doc tests for metrics #1213
- Add `opentelemetry::sdk::logs::config()` for parity with `opentelemetry::sdk::trace::config()` (#1197)

### Changed

- `OtelString::Owned` carries `Box<str>` instead of `String` #1096

### Removed

- Drop include_trace_context parameter from Logs API/SDK. [#1133](https://github.com/open-telemetry/opentelemetry-rust/issues/1133)
- Synchronous instruments no longer accepts `Context` while reporting
  measurements. [#1076](https://github.com/open-telemetry/opentelemetry-rust/pull/1076).
- Fallible conversions from `futures-channel` error types to `LogError` and
  `TraceError` removed.
  [#1201](https://github.com/open-telemetry/opentelemetry-rust/issues/1201)

### Fixed

- Fix `SpanRef::set_attributes` mutability requirement. [#1038](https://github.com/open-telemetry/opentelemetry-rust/pull/1038)
- Move OrderMap module to root of otel-api crate. [#1061](https://github.com/open-telemetry/opentelemetry-rust/pull/1061)
- Use the browser-only js-sys workaround only when actually targeting a browser #1008

## [v0.19.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.18.0...v0.19.0)
This release should been seen as 1.0-rc2 following 1.0-rc1 in v0.18.0. Refer to CHANGELOG.md in individual creates for details on changes made in different creates.

### Added
- Add `WithContext` to public api [#893](https://github.com/open-telemetry/opentelemetry-rust/pull/893).
- Add support for instrumentation scope attributes [#1021](https://github.com/open-telemetry/opentelemetry-rust/pull/1021).

### Changed
- Implement `Display` on `Baggage` [#921](https://github.com/open-telemetry/opentelemetry-rust/pull/921).
- Bump MSRV to 1.57 [#953](https://github.com/open-telemetry/opentelemetry-rust/pull/953).
- Update dependencies and bump MSRV to 1.60 [#969](https://github.com/open-telemetry/opentelemetry-rust/pull/969).

## [v0.18.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.17.0...v0.18.0)

This release is the first beta release of the `trace` API and SDK. If no other
breaking changes are necessary, the next release will be 1.0. The `metrics` API
and SDK are still unstable.

### Added

- Pull sampling probability from `OTEL_TRACES_SAMPLER_ARG` in default sdk config #737
- Add `schema_url` to `Tracer` #743
- Add `schema_url` to `Resource` #775
- Add `Span::set_attributes` #638
- Support concurrent exports #781
- Add jaeger remote sampler #797
- Allow Custom Samplers #833
- Add `SpanExporter::force_flush` and default implementation #845

### Changed

- Deprecate metrics `ValueRecorder` in favor of `Histogram` #728
- Move `IdGenerator` to SDK, rename to `RandomIdGenerator` #742
- `meter_with_version` accepts optional parameter for `version` and `schema_url` #752
- Unify `Event` and `Link` access patterns #757
- move `SpanKind` display format impl to jaeger crate #758
- make `TraceStateError` private #755
- rename `Span::record_exception` to `Span::record_error` #756
- Replace `StatusCode` and `message` with `Status` #760
- Move `TracerProvider::force_flush` to SDK #658
- Switch to static resource references #790
- Allow O(1) get operations for `SpanBuilder::attributes` [breaking] #799
- Allow ref counted keys and values #821
- Bump MSRV from 1.49 to 1.55 #811
- bump MSRV to 1.56 #866
- Update metrics API and SDK for latest spec #819
- Switch to `pin-project-lite` #830

### Fixed

- Update dashmap to avoid soundness hole #818
- Perform sampling as explained in the specification #839
- Remove internal message queue between exporter and exporting tasks #848
- Fix span processor exporting unsampled spans #871

### Removed

- Remove `serialize` feature #738
- Remove `StatusCode::as_str` #741
- Remove `Tracer::with_span` #746

## [v0.17.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.16.0...v0.17.0)

### Changed

- Implement `Serialize` & `Deserialize` for `Sampler`, `SpanLimits` #622, #626
- Allow `&'static str` and `string` in span methods #654
- Allow `String` data in instrumentation library. #670
- Remove `std::fmt::Debug` and `'static` requirements from `TracerProvider`,
  `Tracer`, and `Span` #664
- Remove unused `Tracer::invalid` method #683
- Split `TracerProvider::tracer` and `TracerProvider::versioned_tracer` methods #682
- Reduce dependency on `futures` crate #684
- Switch to parent context references #687
- Spec-compliant trace and span ids #689
- Optimize span creation internals #693
- Add instrumentation library to `ShouldSample` parameters #695

### Fixed

- Fix default resource detection for tracer provider #641
- Detect `service.name` from `OTEL_SERVICE_NAME` and `OTEL_RESOURCE_ATTRIBUTES` #662
- Fix `TraceState::valid_key` crashes #665

## [v0.16.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.15.0...v0.16.0)

### Changed

- Add default resource in `TracerProvider` #571
- Rename `get_tracer` to `tracer` #586
- Extract `trace::noop` module and update docs #587
- Add `Hash` impl for span context and allow spans to clone and export current state #592
- Enforce span status code's order #593
- Make `SpanRef` public #600
- Make `SpanProcessor::on_start` take a mutable span #601
- Renamed `label` to `attribute` to align with otel specification #609

### Performance

- Small performance boost for `Resource::get` #579

## [v0.15.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.14.0...v0.15.0)

### Added

- More resource detectors #573

### Changed

- Expose the Error type to allow users to set custom error handlers #551
- Allow users to use different channels based on runtime in batch span processor #560
- Move `Unit` into `metrics` module #564
- Update trace flags to match spec #565

### Fixed

- Fix debug loop, add notes for `#[tokio::test]` #552
- `TraceState` cannot insert new key-value pairs #567

## [v0.14.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.13.0...v0.14.0)

## Added

- Adding a dynamic dispatch to Aggregator Selector #497
- Add `global::force_flush_tracer_provider` #512
- Add config `max_attributes_per_event` and `max_attributes_per_link` #521
- Add dropped attribute counts to events and links #529

## Changed

- Remove unnecessary clone in `Key` type #491
- Remove `#[must_use]` from `set_tracer_provider` #501
- Rename remaining usage of `default_sampler` to `sampler` #509
- Use current span for SDK-less context propagation #510
- Always export span batch when limit reached #519
- Rename message events to events #530
- Update resource merge behaviour #537
- Ignore links with invalid context #538

## Removed

- Remove remote span context #508
- Remove metrics quantiles #525

# Fixed

- Allow users to use custom export kind selector #526

## Performance

- Improve simple span processor performance #502
- Local span perf improvements #505
- Reduce string allocations where possible #506

## [v0.13.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.12.0...v0.13.0)

Upgrade note: exporter pipelines do not return an uninstall guard as of #444,
use `opentelemetry::global::shutdown_tracer_provider` explicitly instead.

## Changed

- Pull configrations from environment variables by default when creating BatchSpanProcessor #445
- Convert doc links to intra-doc #466
- Switch to Cow for event names #471
- Use API to configure async runtime instead of features #481
- Rename trace config with_default_sampler to with_sampler #482

## Removed

- Removed tracer provider guard #444
- Removed `from_env` and use environment variables to initialize the configurations by default #459

## [v0.12.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.11.2...v0.12.0)

## Added

- Instrumentation library support #402
- Batch observer support #429
- `with_unit` methods in metrics #431
- Clone trait for noop tracer/tracer provider/span #479
- Abstracted traits for different runtimes #480

## Changed

- Dependencies updates #410
- Add `Send`, `Sync` to `AsyncInstrument` in metrics #422
- Add `Send`, `Sync` to `InstrumentCore` in metrics #423
- Replace regex with custom logic #411
- Update tokio to v1 #421

## Removed

- Moved `http` dependencies into a new opentelemetry-http crate #415
- Remove `tonic` dependency #414

## [v0.11.2](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.11.1...v0.11.2)

# Fixed

- Fix possible deadlock when dropping metric instruments #407

## [v0.11.1](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.11.0...v0.11.1)

# Fixed

- Fix remote implicit builder context sampling #405

## [v0.11.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.10.0...v0.11.0)

## Added

- Add `force_flush` method to span processors #358
- Add timeout for `force_flush` and `shutdown` #362

## Changed

- Implement Display trait for Key and Value types #353
- Remove Option from Array values #359
- Update `ShouldSample`'s parent parameter to be `Context` #368
- Consolidate error types in `trace` module into `TraceError` #371
- Add `#[must_use]` to uninstall structs #372
- Move 3rd party propagators and merge exporter into `sdk::export` #375
- Add instrumentation version to instrument config #392
- Use instrumentation library in metrics #393
- `start_from_context` renamed to `start_with_context` #399
- Removed `build_with_context` as full context is now stored in builder #399
- SpanBuilder's `with_parent` renamed to `with_parent_context` #399

# Fixed

- Fix parent based sampling in tracer #354
- StatusCode enum value ordering #377
- Counter adding the delta from last collection #395
- `HistogramAggregator` returning sum vs count #398

## [v0.10.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.9.1...v0.10.0)

## Added

- Add support for baggage metadata #287

## Changed

- Remove `api` prefix from modules #305
- Move `mark_as_active_span` and `get_active_span` functions into trace module #310
- Revert renaming of `SpanContext` to `SpanReference` #299
- Default trace propagator is now a no-op #329
- Return references to span contexts instead of clones #325
- Update exporter errors to be `Box<dyn Error + Send + Sync + 'static>` #284
- Rename `GenericProvider` to `GenericTracerProvider` #313
- Reduce `SpanStatus` enum to `Ok`, `Error`, and `Unset` variants #315
- update B3 propagator to more closely match spec #319
- Export missing pub global trace types #313
- Ensure kv array values are homogeneous #333
- Implement `Display` trait for `Key` and `Value` types #353
- Move `SpanProcessor` trait into `sdk` module #334
- Ensure `is_recording` is `false` and span is no-op after `end` #341
- Move binary propagator and base64 format to contrib #343
- Ensure metrics noop types go through constructors #345
- Change `ExportResult` to use `std::result::Result` #347
- Change `SpanExporter::export` to take `&mut self` instead of `&self` #350
- Add MSRV 1.42.0 #296

## Fixed

- Fix parent based sampling #354

## Removed

- Remove support for `u64` and `bytes` kv values #323
- Remove kv value conversion from `&str` #332

## [v0.9.1](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.9.0...v0.9.1)

## Added

- Allow metric instruments to be cloned #280

### Fixed

- Fix single threaded runtime tokio feature bug #278

## [v0.9.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.8.0...v0.9.0)

## Added

- Add resource detector #174
- Add `fields` method to TextMapFormat #178
- Add support for `tracestate` in `TraceContextPropagator` #191
- Propagate valid span context in noop tracer #197
- Add end_with_timestamp method for trace span #199
- Add ID methods for hex and byte array formatting #200
- Add AWS X-Ray ID Generator #201
- AWS X-Ray Trace Context Propagator #202
- Add instrumentation library information to spans #207
- Add keys method to extractors #209
- Add `TraceState` to `SpanContext` #217
- Add `from_env` config option for `BatchSpanProcessor` #228
- Add pipeline uninstall mechanism to shut down trace pipelines #229

### Changed

- Re-write metrics sdk to be spec compliant #179
- Rename `Sampler::Probability` to `Sampler::TraceIdRatioBased` #188
- Rename `HTTPTextPropagator` to `TextMapPropagator` #192
- Ensure extractors are case insensitive #193
- Rename `Provider` to `TracerProvider` #206
- Rename `CorrelationContext` into `Baggage` #208
- Pipeline builder for stdout trace exporter #224
- Switch to async exporters #232
- Allow `ShouldSample` implementation to modify trace state #237
- Ensure context guard is `!Send` #239
- Ensure trace noop structs use `new` constructor #240
- Switch to w3c `baggage` header #246
- Move trace module imports from `api` to `api::trace` #255
- Update `tonic` feature to use version `0.3.x` #258
- Update exporters to receive owned span data #264
- Move propagators to `sdk::propagation` #266
- Rename SpanContext to SpanReference #270
- Rename `SamplingDecision`'s `NotRecord`, `Record` and `RecordAndSampled` to
  `Drop` `RecordOnly` and `RecordAndSample` #247

## [v0.8.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.7.0...v0.8.0)

## Added

- Add custom span processors to `Provider::Builder` #166

### Changed

- Separate `Carrier` into `Injector` and `Extractor` #164
- Change the default sampler to be `ParentOrElse(AlwaysOn)` #163
- Move the `Sampler` interface to the SDK #169

## [v0.7.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.6.0...v0.7.0)

### Added

- New `ParentOrElse` sampler for fallback logic if parent is not sampled. #128
- Attributes can now have array values #146
- Added `record_exception` and `record_exception_with_stacktrace` methods to `Span` #152

### Changed

- Update sampler types #128
  - `Always` is now `AlwaysOn`. `Never` is now `AlwaysOff`. `Probability` now ignores parent
    sampled state.
- `base64` and `binary_propagator` have been moved to `experimental` module. #134
- `Correlation-Context` header has been updated to `otcorrelations` #145
- `B3Propagator` has been updated to more closely follow the spec #148

## [v0.6.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.5.0...v0.6.0)

### Added

- Add `http` and `tonic` features to impl `Carrier` for common types.

### Changed

- Removed `span_id` from sampling parameters when implementing custom samplers.

### Fixed

- Make `Context` `Send + Sync` in #127

## [v0.5.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.4.0...v0.5.0)

### Added

- Derive `Clone` for `B3Propagator`, `SamplingResult`, and `SpanBuilder`
- Ability to configure the span id / trace id generator
- impl `From<T>` for common `Key` and `Value` types
- Add global `tracer` method
- Add `Resource` API
- Add `Context` API
- Add `Correlations` API
- Add `HttpTextCompositePropagator` for composing `HttpTextPropagator`s
- Add `GlobalPropagator` for globally configuring a propagator
- Add `TraceContextExt` to provide methods for working with trace data in a context
- Expose `EvictedQueue` constructor

### Changed

- Ensure that impls of `Span` are `Send` and `Sync` when used in `global`
- Changed `Key` and `Value` method signatures to remove `Cow` references
- Tracer's `start` now uses the implicit current context instead of an explicit span context.
  `start_with_context` may be used to specify a context if desired.
- `with_span` now accepts a span for naming consistency and managing the active state of a more
  complex span (likely produced by a builder), and the previous functionality that accepts a
  `&str` has been renamed to `in_span`, both of which now yield a context to the provided closure.
- Tracer's `get_active_span` now accepts a closure
- The `Instrument` trait has been renamed to `FutureExt` to avoid clashing with metric instruments,
  and instead accepts contexts via `with_context`.
- Span's `get_context` method has been renamed to `span_context` to avoid ambiguity.
- `HttpTextPropagators` inject the current context instead of an explicit span context. The context
  can be specified with `inject_context`.
- `SpanData`'s `context` has been renamed to `span_context`

### Fixed

- Update the probability sampler to match the spec
- Rename `Traceparent` header to `traceparent`

### Removed

- `TracerGenerics` methods have been folded in to the `Tracer` trait so it is longer needed
- Tracer's `mark_span_as_inactive` has been removed
- Exporters no longer require an `as_any` method
- Span's `mark_as_active`, `mark_as_inactive`, and `as_any` have been removed

## [v0.4.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.3.0...v0.4.0)

### Added

- New async batch span processor
- New stdout exporter
- Add `trace_id` to `SpanBuilder`

### Changed

- Add `attributes` to `Event`s.
- Update `Span`'s `add_event` and `add_event_with_timestamp` to accept attributes.
- Record log fields in jaeger exporter
- Properly export span kind in jaeger exporter
- Add support for `Link`s
- Add `status_message` to `Span` and `SpanData`
- Rename `SpanStatus` to `StatusCode`
- Update `EvictedQueue` internals from LIFO to FIFO
- Switch span attributes to `EvictedHashMap`

### Fixed

- Call `shutdown` correctly when span processors and exporters are dropped

## [v0.3.0](https://github.com/open-telemetry/opentelemetry-rust/compare/v0.2.0...v0.3.0)

### Added

- New Base64 propagator
- New SpanBuilder api
- Zipkin Exporter crate

### Changed

- Switch to `SpanId` and `TraceId` from `u64` and `u128`
- Remove `&mut self` requirements for `Span` API

### Fixed

- circular Tracer debug impl

## [v0.2.0](https://github.com/open-telemetry/opentelemetry-rust/compare/b5918251cc07f9f6957434ccddc35306f68bd791..v0.2.0)

### Added

- Make trace and metrics features optional
- ExportResult as specified in the specification
- Add Futures compatibility API
- Added serde serialise support to SpanData
- Separate OpenTelemetry Jaeger crate

### Changed

- Rename HttpTraceContextPropagator to TraceContextPropagator
- Rename HttpB3Propagator to B3Propagator
- Switch to Apache 2 license
- Resolve agent addresses to allow non-static IP
- Remove tracer name prefix from span name

### Removed

- Remove add_link from spans

## [v0.1.5](https://github.com/jtescher/opentelemetry-rust/compare/v0.1.4...v0.1.5)

### Added

- trace-context propagator

### Changed

- Prometheus API cleanup

## [v0.1.4](https://github.com/jtescher/opentelemetry-rust/compare/v0.1.3...v0.1.4)

### Added

- Parent option for default sampler

### Fixed

- SDK tracer default span id

## [v0.1.3](https://github.com/jtescher/opentelemetry-rust/compare/v0.1.2...v0.1.3)

### Changed

- Ensure spans are always send and sync
- Allow static lifetimes for span names
- Improve KeyValue ergonomics

## [v0.1.2](https://github.com/jtescher/opentelemetry-rust/compare/v0.1.1...v0.1.2)

### Added

- Implement global provider

## [v0.1.1](https://github.com/jtescher/opentelemetry-rust/compare/v0.1.0...v0.1.1)

### Added

- Documentation and API cleanup
- Tracking of active spans via thread local span stack

## [v0.1.0](https://github.com/jtescher/opentelemetry-rust/commit/ea368ea965aa035f46728d75e1be3b096b6cd6ec)

Initial debug alpha
