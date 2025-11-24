# Changelog

## vNext

## 0.30.0

Released 2025-May-23

- Updated `opentelemetry` and `opentelemetry-http` dependencies to version 0.30.0.
- It is now possible to add links to a `Span` via the `SpanRef` that you get from
  a `Context`. [2959](https://github.com/open-telemetry/opentelemetry-rust/pull/2959)
- **Feature**: Added context based telemetry suppression. [#2868](https://github.com/open-telemetry/opentelemetry-rust/pull/2868)
  - `SdkLogger`, `SdkTracer` modified to respect telemetry suppression based on
`Context`. In other words, if the current context has telemetry suppression
enabled, then logs/spans will be ignored.
  - The flag is typically set by OTel
components to prevent telemetry from itself being fed back into OTel.
  - `BatchLogProcessor`, `BatchSpanProcessor`, and `PeriodicReader` modified to set
the suppression flag in their dedicated thread, so that telemetry generated from
those threads will not be fed back into OTel.
  - Similarly, `SimpleLogProcessor`
also modified to suppress telemetry before invoking exporters.

- **Feature**: Implemented and enabled cardinality capping for Metrics by
  default. [#2901](https://github.com/open-telemetry/opentelemetry-rust/pull/2901)
  - The default cardinality limit is 2000 and can be customized using Views.  
  - This feature was previously removed in version 0.28 due to the lack of
    configurability but has now been reintroduced with the ability to configure
    the limit.
  - Fixed the overflow attribute to correctly use the boolean value `true`
    instead of the string `"true"`.
    [#2878](https://github.com/open-telemetry/opentelemetry-rust/issues/2878)
- The `shutdown_with_timeout` method is added to SpanProcessor, SpanExporter trait and TracerProvider.
- The `shutdown_with_timeout` method is added to LogExporter trait.
- The `shutdown_with_timeout` method is added to LogProvider and LogProcessor trait.
- *Breaking* `MetricError`, `MetricResult` no longer public (except when
  `spec_unstable_metrics_views` feature flag is enabled). `OTelSdkResult` should
  be used instead, wherever applicable. [#2906](https://github.com/open-telemetry/opentelemetry-rust/pull/2906)
- *Breaking* change, affecting custom `MetricReader` authors:
  - The
  `shutdown_with_timeout` method is added to `MetricReader` trait.
  - `collect`
  method on `MetricReader` modified to return `OTelSdkResult`.
  [#2905](https://github.com/open-telemetry/opentelemetry-rust/pull/2905)
  - `MetricReader`
  trait, `ManualReader` struct, `Pipeline` struct, `InstrumentKind` enum moved
  behind feature flag "experimental_metrics_custom_reader".
  [#2928](https://github.com/open-telemetry/opentelemetry-rust/pull/2928)

- **Views improvements**:
  - Core view functionality is now available by default—users can change the
    name, unit, description, and cardinality limit of a metric via views without
    enabling the `spec_unstable_metrics_views` feature flag. Advanced view
    features, such as custom aggregation or attribute filtering, still require
    the `spec_unstable_metrics_views` feature.
  - Removed `new_view()` method and `View` trait. Views can now be added by passing
    a function with signature `Fn(&Instrument) -> Option<Stream>` to the `with_view`
    method on `MeterProviderBuilder`.
- Introduced a builder pattern for `Stream` creation to use with views:
  - Added `StreamBuilder` struct with methods to configure stream properties
  - Added `Stream::builder()` method that returns a new `StreamBuilder`
  - `StreamBuilder::build()` returns `Result<Stream, Box<dyn Error>>` enabling
    proper validation.

Example of using views to rename a metric:

```rust
let view_rename = |i: &Instrument| {
    if i.name() == "my_histogram" {
        Some(
            Stream::builder()
                .with_name("my_histogram_renamed")
                .build()
                .unwrap(),
        )
    } else {
        None
    }
};

let provider = SdkMeterProvider::builder()
    // add exporters, set resource etc.
    .with_view(view_rename)
    .build();
```

- *Breaking* `Aggregation` enum moved behind feature flag
  "spec_unstable_metrics_views". This was only required when using advanced view
  capabilities.
  [#2928](https://github.com/open-telemetry/opentelemetry-rust/pull/2928)
- *Breaking* change, affecting custom `PushMetricExporter` authors:
  - The `export` method on `PushMetricExporter` now accepts `&ResourceMetrics`
    instead of `&mut ResourceMetrics`.
  - `ResourceMetrics` no longer exposes `scope_metrics` field, but instead
    offers `scope_metrics()` method that returns an iterator over the same.
  - `ScopeMetrics` no longer exposes `metrics` field, but instead offers
    `metrics()` method that returns an iterator over the same.
  - `Sum`, `Gauge`, `Histogram` & `ExponentialHistogram` no longer exposes
    `data_points` field, but instead offers `data_points()` method that returns
    an iterator over the same.
  - `SumDataPoint`, `GaugeDataPoint`, `HistogramDataPoint` &
    `ExponentialHistogramDataPoint` no longer exposes `attributes`, `exemplars`
    field, but instead offers `attributes()`, and `exemplars()` method that
    returns an iterator over the same.
  - `Exemplar` no longer exposes `filtered_attributes` field, but instead
     offers `filtered_attributes()` method that returns an iterator over
     the same.
  - `HistogramDataPoint` no longer exposes `bounds` and `bucket_counts`, but
    instead offers `bounds()` and `bucket_counts()` methods that returns an
    iterator over the same.
  - `Metric` no longer exposes `name`, `description`, `unit`, `data` fields, but
    instead offers `name()`, `description()`, `unit()`, and `data()` accessor methods.
  - `ResourceMetrics` no longer exposes `resource` field, but instead offers
    a `resource()` accessor method.
  - `ScopeMetrics` no longer exposes `scope` field, but instead offers
    a `scope()` accessor method.

## 0.29.0

Released 2025-Mar-21

- Update `opentelemetry` dependency to 0.29.
- Update `opentelemetry-http` dependency to 0.29.
- **Breaking**: The `Runtime` trait has been simplified and refined. See the [#2641](https://github.com/open-telemetry/opentelemetry-rust/pull/2641)
  for the changes.
- Removed `async-std` support for `Runtime`, as [`async-std` crate is deprecated](https://crates.io/crates/async-std).
- Calls to `MeterProviderBuilder::with_resource`, `TracerProviderBuilder::with_resource`,
  `LoggerProviderBuilder::with_resource` are now additive ([#2677](https://github.com/open-telemetry/opentelemetry-rust/pull/2677)).
- Moved `ExportError` trait from `opentelemetry::trace::ExportError` to `opentelemetry_sdk::export::ExportError`
- Moved `TraceError` enum from `opentelemetry::trace::TraceError` to `opentelemetry_sdk::trace::TraceError`
- Moved `TraceResult` type alias from `opentelemetry::trace::TraceResult` to `opentelemetry_sdk::trace::TraceResult`
- **Breaking**: Make `force_flush()` in `PushMetricExporter` synchronous
- **Breaking**: Updated the `SpanExporter` trait method signature:

```rust
  fn export(&mut self, batch: Vec<SpanData>) -> BoxFuture<'static, OTelSdkResult>;
```

  to

```rust
  fn export(
    &mut self,
    batch: Vec<SpanData>,
) -> impl std::future::Future<Output = OTelSdkResult> + Send;
```

  This affects anyone who writes custom exporters, as custom implementations of SpanExporter
  should now define export as an `async fn`:

```rust
  impl trace::SpanExporter for CustomExporter {
    async fn export(&mut self, batch: Vec<trace::SpanData>) -> OTelSdkResult {
        // Implementation here
    }
}
```

- **Breaking** The SpanExporter::export() method no longer requires a mutable reference to self.
  Before:

  ```rust
    async fn export(&mut self, batch: Vec<SpanData>) -> OTelSdkResult
  ```

  After:
  
  ```rust
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult
  ```

  Custom exporters will need to internally synchronize any mutable state, if applicable.

- **Breaking** The `shutdown_with_timeout` method is added to MetricExporter trait. This is breaking change for custom `MetricExporter` authors.
- Bug Fix: `BatchLogProcessor` now correctly calls `shutdown` on the exporter
  when its `shutdown` is invoked.

- Reduced some info level logs to debug
- **Breaking** for custom LogProcessor/Exporter authors: Changed `name`
  parameter from `&str` to `Option<&str>` in `event_enabled` method on the
  `LogProcessor` and `LogExporter` traits. `SdkLogger` no longer passes its
  `scope` name but instead passes the incoming `name` when invoking
  `event_enabled` on processors.
- **Breaking** for custom LogExporter authors: `shutdown()` method in
 `LogExporter` trait no longer requires a mutable ref to `self`. If the exporter
 needs to mutate state, it should rely on interior mutability.
 [2764](https://github.com/open-telemetry/opentelemetry-rust/pull/2764)
- *Breaking (Affects custom Exporter/Processor authors only)* Removed
 `opentelelemetry_sdk::logs::error::{LogError, LogResult}`. These were not
 intended to be public. If you are authoring custom processor/exporters, use
 `opentelemetry_sdk::error::OTelSdkError` and
 `opentelemetry_sdk::error::OTelSdkResult`.
  [2790](https://github.com/open-telemetry/opentelemetry-rust/pull/2790)
- **Breaking** for custom `LogProcessor` authors: Changed `set_resource`
  to require mutable ref.
  `fn set_resource(&mut self, _resource: &Resource) {}`
- **Breaking**: InMemoryExporter's return type change.
  - `TraceResult<Vec<SpanData>>` to `Result<Vec<SpanData>, InMemoryExporterError>`
  - `MetricResult<Vec<ResourceMetrics>>` to `Result<Vec<ResourceMetrics>, InMemoryExporterError>`
  - `LogResult<Vec<LogDataWithResource>>` to `Result<Vec<LogDataWithResource>, InMemoryExporterError>`

## 0.28.0

Released 2025-Feb-10

Note: Due to the large amount of making changes, check [migration guide to
0.28](../docs/migration_0.28.md) for a summary that can help majority users to
quickly migrate. The changelog below is the full list of changes.

- Update `opentelemetry` dependency to 0.28.
- Update `opentelemetry-http` dependency to 0.28.
- Bump msrv to 1.75.0.
- *Bug fix*: For cumulative temporality, ObservableGauge no longer export
  MetricPoints unless measurements were newly reported (in Observable callbacks)
  since last export. This bug fixes ensures ObservableGauge behaves as per OTel
  Spec. The bug is *not* addressed for other Observable instruments
  [#2213](https://github.com/open-telemetry/opentelemetry-rust/issues/2213)
- Upgrade the tracing crate used for internal logging to version 0.1.40 or
later. This is necessary because the internal logging macros utilize the name
field as metadata, a feature introduced in version 0.1.40.
[#2418](https://github.com/open-telemetry/opentelemetry-rust/pull/2418)
- *Feature*: Introduced a new feature flag,
  `experimental_metrics_disable_name_validation`, which disables entire
  Instrument Name Validation. This is an experimental feature to unblock use
  cases requiring currently disallowed characters (eg: Windows Perf Counters).
  Use caution when enabling this feature as this breaks guarantees about metric
  name.
- Bug fix: Empty Tracer names are retained as-is instead of replacing with
  "rust.opentelemetry.io/sdk/tracer"
  [#2486](https://github.com/open-telemetry/opentelemetry-rust/pull/2486)
- Update `EnvResourceDetector` to allow resource attribute values containing
  equal signs (`"="`).
  [#2120](https://github.com/open-telemetry/opentelemetry-rust/pull/2120)
- `ResourceDetector.detect()` no longer supports timeout option.
- *Breaking* Resource.get() modified to require reference to Key instead of
  owned. Replace `get(Key::from_static_str("key"))` with
  `get(&Key::from_static_str("key"))`
- *Feature*: Add `ResourceBuilder` for an easy way to create new `Resource`s
- *Breaking*: Remove
- `Resource::{new,empty,from_detectors,new_with_defaults,from_schema_url,merge,default}`.
   To create Resources you should only use `Resource::builder()` or `Resource::builder_empty()`. See
   [#2322](https://github.com/open-telemetry/opentelemetry-rust/pull/2322) for a migration guide.

  Example Usage:

  ```rust
  // old
  Resource::default().with_attributes([
      KeyValue::new("service.name", "test_service"),
      KeyValue::new("key", "value"),
  ]);

  // new
  Resource::builder()
      .with_service_name("test_service")
      .with_attribute(KeyValue::new("key", "value"))
      .build();
  ```

- *Breaking* :
  [#2314](https://github.com/open-telemetry/opentelemetry-rust/pull/2314)
  - The LogRecord struct has been updated:
    - All fields are now pub(crate) instead of pub.
    - Getter methods have been introduced to access field values. This change
    impacts custom exporter and processor developers by requiring updates to
    code that directly accessed LogRecord fields. They must now use the provided
    getter methods (e.g., `log_record.event_name()` instead of
    `log_record.event_name`).
- *Breaking (Affects custom metric exporter authors only)* `start_time` and
  `time` is moved from DataPoints to aggregations (Sum, Gauge, Histogram,
  ExpoHistogram) see
  [#2377](https://github.com/open-telemetry/opentelemetry-rust/pull/2377) and
  [#2411](https://github.com/open-telemetry/opentelemetry-rust/pull/2411), to
  reduce memory.
- *Breaking* `start_time` is no longer optional for `Sum` aggregation, see
  [#2367](https://github.com/open-telemetry/opentelemetry-rust/pull/2367), but
  is still optional for `Gauge` aggregation see
  [#2389](https://github.com/open-telemetry/opentelemetry-rust/pull/2389).
- SimpleLogProcessor modified to be generic over `LogExporter` to avoid
  dynamic dispatch to invoke exporter. If you were using
  `with_simple_exporter` to add `LogExporter` with SimpleLogProcessor, this is
  a transparent change.
  [#2338](https://github.com/open-telemetry/opentelemetry-rust/pull/2338)
- *Breaking* `opentelemetry::global::shutdown_tracer_provider()` removed from the API,
  should now use `tracer_provider.shutdown()` see
  [#2369](https://github.com/open-telemetry/opentelemetry-rust/pull/2369) for
  a migration example. "Tracer provider" is cheaply clonable, so users are
  encouraged to set a clone of it as the global (ex:
  `global::set_tracer_provider(provider.clone()))`, so that instrumentations
  and other components can obtain tracers from `global::tracer()`. The
  tracer_provider must be kept around to call shutdown on it at the end of
  application (ex: `tracer_provider.shutdown()`)

- *Breaking* The LogExporter::export() method no longer requires a mutable
  reference to self.: Before: `async fn export(&mut self, _batch: LogBatch<'_>)
     -> LogResult<()>` After: `async fn export(&self, _batch: LogBatch<'_>) ->
  LogResult<()>` Custom exporters will need to internally synchronize any
     mutable state, if applicable.

- *Breaking* Removed the following deprecated struct:
  - logs::LogData - Previously deprecated in version 0.27.1 Migration Guidance:
  This structure is no longer utilized within the SDK, and users should not have
  dependencies on it.

- *Breaking* Removed the following deprecated methods:
  - `Logger::provider()` : Previously deprecated in version 0.27.1
  - `Logger::instrumentation_scope()` : Previously deprecated in version 0.27.1.
     Migration Guidance: - These methods were intended for log appender authors.
        Keep the clone of the provider handle, instead of depending on above
        methods.
- Rename `opentelemetry_sdk::logs::Builder` to
  `opentelemetry_sdk::logs::LoggerProviderBuilder`.
- Rename `opentelemetry_sdk::trace::Builder` to
  `opentelemetry_sdk::trace::SdkTracerProviderBuilder`.
- Redesigned PeriodicReader, BatchSpanProcessor, BatchLogProcessor to no longer
  require an async runtime. They create its own background thread instead. When
  pairing with OTLP, `grpc-tonic` or `reqwest-blocking-client` are the only
  supported features (`hyper`, `reqwest` are not supported) These are now
  enabled by default and can be migrated to by removing the extra `rt:Runtime`
  argument as shown below.
  - `PeriodicReader::builder(exporter,runtime::Tokio).build();` to
    `PeriodicReader::builder(exporter).build();`
  - `.with_batch_exporter(exporter, runtime::Tokio)` to
    `.with_batch_exporter(exporter)`

  The new implementation has following limitations:
  - Does not work if your application cannot spawn new Thread.
  - Does not support `hyper`, `reqwest` HTTP Clients
  - Does not support multiple concurrent exports (`with_max_concurrent_exports`
     is not supported). This existed only for traces.

  If this applies to you, you can get the old behavior back by following steps
  below:
  - Enable one or more of the feature flag from below
   `experimental_metrics_periodicreader_with_async_runtime`
   `experimental_logs_batch_log_processor_with_async_runtime`
   `experimental_trace_batch_span_processor_with_async_runtime`
  - Use updated namespace; i.e
  `periodic_reader_with_async_runtime::PeriodicReader`,
  `log_processor_with_async_runtime::BatchLogProcessor` and
  `span_processor_with_async_runtime::BatchSpanProcessor`
  - Continue using existing features flags `rt-tokio`,
      `rt-tokio-current-thread`, or `rt-async-std`.

  As part of the above redesign of PeriodicReader and BatchProcessors, these
  components no longer enforce timeout themselves and instead relies on
  Exporters to enforce own timeouts. In other words, the following are no longer
  supported.
  - `with_max_export_timeout`, `with_timeout` methods on `BatchConfigBuilder`,
    `PeriodicReaderBuilder`
  - `OTEL_BLRP_EXPORT_TIMEOUT`, `OTEL_BSP_EXPORT_TIMEOUT`

  Users are advised to configure timeout on the Exporters itself. For example,
  in the OTLP exporter, the export timeout can be configured using:
  - Environment variables
    - `OTEL_EXPORTER_OTLP_TIMEOUT`
    - `OTEL_EXPORTER_OTLP_LOGS_TIMEOUT`, `OTEL_EXPORTER_OTLP_TRACES_TIMEOUT`,
      `OTEL_EXPORTER_OTLP_METRICS_TIMEOUT`
  - The opentelemetry_otlp API, via `.with_tonic().with_timeout()` or
    `.with_http().with_timeout()`.

- *Breaking* Introduced `experimental_async_runtime` feature for
  runtime-specific traits.
  - Runtime-specific features (`rt-tokio`, `rt-tokio-current-thread`, and
  `rt-async-std`) now depend on the `experimental_async_runtime` feature.
  - For most users, no action is required. Enabling runtime features such as
  `rt-tokio`, `rt-tokio-current-thread`, or `rt-async-std` will automatically
  enable the `experimental_async_runtime` feature.
  - If you're implementing a custom runtime, you must explicitly enable the
  experimental_async_runtime` feature in your Cargo.toml and implement the
  required `Runtime` traits.

- Removed Metrics Cardinality Limit feature. This was originally introduced in
[#1066](https://github.com/open-telemetry/opentelemetry-rust/pull/1066) with a
hardcoded limit of 2000 and no ability to change it. This feature will be
re-introduced in a future date, along with the ability to change the cardinality
limit.

- *Breaking* Removed unused `opentelemetry_sdk::Error` enum.
- *Breaking* (Affects custom Exporter authors only) Moved `ExportError` trait
  from `opentelemetry::export::ExportError` to `opentelemetry_sdk::ExportError`
- *Breaking (Affects custom SpanExporter, SpanProcessor authors only)*: Rename
  namespaces for Span exporter structs/traits before:
  `opentelemetry_sdk::export::spans::{ExportResult, SpanData, SpanExporter};`
  now: `opentelemetry_sdk::trace::{ExportResult, SpanData, SpanExporter};`

- *Breaking (Affects custom LogExporter, LogProcessor authors only)*: Rename
  namespaces for Log exporter structs/traits. before:
  `opentelemetry_sdk::export::logs::{ExportResult, LogBatch, LogExporter};` now:
  `opentelemetry_sdk::logs::{ExportResult, LogBatch, LogExporter};`

- *Breaking* `opentelemetry_sdk::LogRecord::default()` method is removed. The
  only way to create log record outside opentelemetry_sdk crate is using
  `Logger::create_log_record()` method.

- *Breaking*: Rename namespaces for InMemoryExporters. (The module is still
  under "testing" feature flag)
  before:

  ```rust
  opentelemetry_sdk::testing::logs::{InMemoryLogExporter,
  InMemoryLogExporterBuilder};
  opentelemetry_sdk::testing::trace::{InMemorySpanExporter,
  InMemorySpanExporterBuilder};
  opentelemetry_sdk::testing::metrics::{InMemoryMetricExporter,
  InMemoryMetricExporterBuilder};
  ```

  now:

  ```rust
  opentelemetry_sdk::logs::{InMemoryLogExporter, InMemoryLogExporterBuilder};
  opentelemetry_sdk::trace::{InMemorySpanExporter,
  InMemorySpanExporterBuilder};
  opentelemetry_sdk::metrics::{InMemoryMetricExporter,
  InMemoryMetricExporterBuilder};
  ```

- *Breaking* Renamed `LoggerProvider`, `Logger` and `LogRecord` to
  `SdkLoggerProvider`,`SdkLogger` and `SdkLogRecord` respectively to avoid name
  collision with public API types.
  [#2612](https://github.com/open-telemetry/opentelemetry-rust/pull/2612)

- *Breaking* Renamed `TracerProvider` and `Tracer` to `SdkTracerProvider` and
  `SdkTracer` to avoid name collision with public API types. `Tracer` is still
  type-aliased to `SdkTracer` to keep back-compat with tracing-opentelemetry.
  [#2614](https://github.com/open-telemetry/opentelemetry-rust/pull/2614)

- *Breaking* Providers, Exporters, Processors, and Readers are modified to use a
  unified Result type for `export()`, `force_flush()`, and `shutdown()` methods.
  All these methods now use `OTelSdkResult` as their return type. Following PRs
  show the exact changes:
  [2613](https://github.com/open-telemetry/opentelemetry-rust/pull/2613)
    [2625](https://github.com/open-telemetry/opentelemetry-rust/pull/2625)
    [2604](https://github.com/open-telemetry/opentelemetry-rust/pull/2604)
    [2606](https://github.com/open-telemetry/opentelemetry-rust/pull/2606)
    [2573](https://github.com/open-telemetry/opentelemetry-rust/pull/2573)

## 0.27.1

Released 2024-Nov-27

- **DEPRECATED**:
  - `trace::Config` methods are moving onto `TracerProvider` Builder to be consistent with other signals. See <https://github.com/open-telemetry/opentelemetry-rust/pull/2303> for migration guide.
    `trace::Config` is scheduled to be removed from public API in `v0.28.0`.
    example:

    ```rust
    // old
    let tracer_provider: TracerProvider = TracerProvider::builder()
        .with_config(Config::default().with_resource(Resource::empty()))
        .build();

    // new
    let tracer_provider: TracerProvider = TracerProvider::builder()
        .with_resource(Resource::empty())
        .build();
    ```

  - `logs::LogData` struct is deprecated, and scheduled to be removed from public API in `v0.28.0`.
  - Bug fix: Empty Meter names are retained as-is instead of replacing with
    "rust.opentelemetry.io/sdk/meter"
    [#2334](https://github.com/open-telemetry/opentelemetry-rust/pull/2334)

  - Bug fix: Empty Logger names are retained as-is instead of replacing with
    "rust.opentelemetry.io/sdk/logger"
    [#2316](https://github.com/open-telemetry/opentelemetry-rust/pull/2316)

  - `Logger::provider`: This method is deprecated as of version `0.27.1`. To be removed in `0.28.0`.
  - `Logger::instrumentation_scope`: This method is deprecated as of version `0.27.1`. To be removed in `0.28.0`
     Migration Guidance:
        - These methods are intended for log appenders. Keep the clone of the provider handle, instead of depending on above methods.

  - **Bug Fix:** Validates the `with_boundaries` bucket boundaries used in
    Histograms. The boundaries provided by the user must not contain `f64::NAN`,
    `f64::INFINITY` or `f64::NEG_INFINITY` and must be sorted in strictly
    increasing order, and contain no duplicates. Instruments will not record
    measurements if the boundaries are invalid.
    [#2351](https://github.com/open-telemetry/opentelemetry-rust/pull/2351)

- Added `with_periodic_exporter` method to `MeterProviderBuilder`, allowing
  users to easily attach an exporter with a PeriodicReader for automatic metric
  export. Retained with_reader() for advanced use cases where a custom
  MetricReader configuration is needed.
  [2597](https://github.com/open-telemetry/opentelemetry-rust/pull/2597)
  Example Usage:

  ```rust
  SdkMeterProvider::builder()
      .with_periodic_exporter(exporter)
      .build();
  ```

  Using a custom PeriodicReader (advanced use case):

  let reader = PeriodicReader::builder(exporter).build();
  SdkMeterProvider::builder()
      .with_reader(reader)
      .build();

## 0.27.0

Released 2024-Nov-11

- Update `opentelemetry` dependency version to 0.27
- Update `opentelemetry-http` dependency version to 0.27

- Bump MSRV to 1.70 [#2179](https://github.com/open-telemetry/opentelemetry-rust/pull/2179)
- Implement `LogRecord::set_trace_context` for `LogRecord`. Respect any trace context set on a `LogRecord` when emitting through a `Logger`.
- Improved `LoggerProvider` shutdown handling to prevent redundant shutdown calls when `drop` is invoked. [#2195](https://github.com/open-telemetry/opentelemetry-rust/pull/2195)
- When creating new metric instruments by calling `build()`, SDK would return a no-op instrument if the validation fails (eg: Invalid metric name). [#2166](https://github.com/open-telemetry/opentelemetry-rust/pull/2166)
- **BREAKING for Metrics users**:
  - **Replaced**
    - ([#2217](https://github.com/open-telemetry/opentelemetry-rust/pull/2217)): Removed `{Delta,Cumulative}TemporalitySelector::new()` in favor of directly using `Temporality` enum to simplify the configuration of MetricsExporterBuilder with different temporalities.
  - **Renamed**
    - ([#2232](https://github.com/open-telemetry/opentelemetry-rust/pull/2232)): The `init` method used to create instruments has been renamed to `build`.
      Before:

      ```rust
      let counter = meter.u64_counter("my_counter").init();
      ```

      Now:

      ```rust
      let counter = meter.u64_counter("my_counter").build();
      ```

    - ([#2255](https://github.com/open-telemetry/opentelemetry-rust/pull/2255)): de-pluralize Metric types.
      - `PushMetricsExporter` -> `PushMetricExporter`
      - `InMemoryMetricsExporter` -> `InMemoryMetricExporter`
      - `InMemoryMetricsExporterBuilder` -> `InMemoryMetricExporterBuilder`
- **BREAKING**: [#2220](https://github.com/open-telemetry/opentelemetry-rust/pull/2220)
  - Removed `InstrumentationLibrary` re-export and its `Scope` alias, use `opentelemetry::InstrumentationLibrary` instead.
  - Unified builders across signals
    - Removed deprecated `LoggerProvider::versioned_logger`, `TracerProvider::versioned_tracer`
    - Removed `MeterProvider::versioned_meter`
    - Replaced these methods with `LoggerProvider::logger_with_scope`, `TracerProvider::logger_with_scope`, `MeterProvider::meter_with_scope`

- [#2272](https://github.com/open-telemetry/opentelemetry-rust/pull/2272)
  - Pin url version to `2.5.2`. The higher version breaks the build refer: [servo/rust-url#992.](https://github.com/servo/rust-url/issues/992)
   The `url` crate is used when `jaeger_remote_sampler` feature is enabled.

- **BREAKING**: [#2266](https://github.com/open-telemetry/opentelemetry-rust/pull/2266)
  - Moved `ExportError` trait from `opentelemetry::ExportError` to `opentelemetry_sdk::export::ExportError`
  - Moved `LogError` enum from `opentelemetry::logs::LogError` to `opentelemetry_sdk::logs::LogError`
  - Moved `LogResult` type alias from `opentelemetry::logs::LogResult` to `opentelemetry_sdk::logs::LogResult`
  - Renamed `opentelemetry::metrics::Result` type alias to `opentelemetry::metrics::MetricResult`
  - Renamed `opentelemetry::metrics::MetricsError` enum to `opentelemetry::metrics::MetricError`
  - Moved `MetricError` enum from `opentelemetry::metrics::MetricError` to `opentelemetry_sdk::metrics::MetricError`
  - Moved `MetricResult` type alias from `opentelemetry::metrics::MetricResult` to `opentelemetry_sdk::metrics::MetricResult`

  - Users calling public APIs that return these constructs (e.g, LoggerProvider::shutdown(), MeterProvider::force_flush()) should now import them from the SDK instead of the API.
  - Developers creating custom exporters should ensure they import these constructs from the SDK, not the API.
  - [2291](https://github.com/open-telemetry/opentelemetry-rust/pull/2291) Rename `logs_level_enabled flag` to `spec_unstable_logs_enabled`. Please enable this updated flag if the feature is needed. This flag will be removed once the feature is stabilized in the specifications.

- **BREAKING**: `Temporality` enum moved from `opentelemetry_sdk::metrics::data::Temporality` to `opentelemetry_sdk::metrics::Temporality`.

- **BREAKING**: `Views` are now an opt-in ONLY feature. Please include the feature `spec_unstable_metrics_views` to enable `Views`. It will be stabilized post 1.0 stable release of the SDK. [#2295](https://github.com/open-telemetry/opentelemetry-rust/issues/2295)

- Added a new `PeriodicReader` implementation (`PeriodicReaderWithOwnThread`)
  that does not rely on an async runtime, and instead creates own Thread. This
  is under feature flag "experimental_metrics_periodic_reader_no_runtime". The
  functionality maybe moved into existing PeriodReader or even removed in the
  future. As of today, this cannot be used as-is with OTLP Metric Exporter or
  any exporter that require an async runtime.

## v0.26.0

Released 2024-Sep-30

- Update `opentelemetry` dependency version to 0.26
- **BREAKING** Public API changes:
  - **Removed**: `SdkMeter` struct [#2113](https://github.com/open-telemetry/opentelemetry-rust/pull/2113). This API is only meant for internal use.
  - **Removed**: `AggregationSelector` trait and `DefaultAggregationSelector` struct [#2085](https://github.com/open-telemetry/opentelemetry-rust/pull/2085). This API was unnecessary. The feature to customize aggregation for instruments should be offered by `Views` API.

- Update `async-std` dependency version to 1.13
- *Breaking* - Remove support for `MetricProducer` which allowed metrics from
  external sources to be sent through OpenTelemetry.
  [#2105](https://github.com/open-telemetry/opentelemetry-rust/pull/2105)
- Feature: `SimpleSpanProcessor::new` is now public [#2119](https://github.com/open-telemetry/opentelemetry-rust/pull/2119)
- For Delta Temporality, exporters are not invoked unless there were new
  measurements since the last collect/export.
  [#2153](https://github.com/open-telemetry/opentelemetry-rust/pull/2153)
- `MeterProvider` modified to not invoke shutdown on `Drop`, if user has already
  called `shutdown()`.
  [#2156](https://github.com/open-telemetry/opentelemetry-rust/pull/2156)

## v0.25.0

- Update `opentelemetry` dependency version to 0.25
- Starting with this version, this crate will align with `opentelemetry` crate
  on major,minor versions.
- Perf improvements for all metric instruments (except `ExponentialHistogram`) that led to **faster metric updates** and **higher throughput** [#1740](https://github.com/open-telemetry/opentelemetry-rust/pull/1740):
  - **Zero allocations when recording measurements**: Once a measurement for a given attribute combination is reported, the SDK would not allocate additional memory for subsequent measurements reported for the same combination.
  - **Minimized thread contention**: Threads reporting measurements for the same instrument no longer contest for the same `Mutex`. The internal aggregation data structure now uses a combination of `RwLock` and atomics. Consequently, threads reporting measurements now only have to acquire a read lock.
  - **Lock-free floating point updates**: Measurements reported for `f64` based metrics no longer need to acquire a `Mutex` to update the `f64` value. They use a CAS-based loop instead.

- `opentelemetry_sdk::logs::record::LogRecord` and `opentelemetry_sdk::logs::record::TraceContext` derive from `PartialEq` to facilitate Unit Testing.
- Fixed an issue causing a panic during shutdown when using the
  `TokioCurrentThread` in BatchExportProcessor for traces and logs.
  [#1964](https://github.com/open-telemetry/opentelemetry-rust/pull/1964)
  [#1973](https://github.com/open-telemetry/opentelemetry-rust/pull/1973)
- Fix BatchExportProcessor for traces and logs to trigger first export at the
  first interval instead of doing it right away.
  [#1970](https://github.com/open-telemetry/opentelemetry-rust/pull/1970)
  [#1973](https://github.com/open-telemetry/opentelemetry-rust/pull/1973)
  - *Breaking* [#1985](https://github.com/open-telemetry/opentelemetry-rust/pull/1985)
  Hide LogRecord attributes Implementation Details from processors and exporters.
  The custom exporters and processors can't directly access the `LogData::LogRecord::attributes`, as
  these are private to opentelemetry-sdk. Instead, they would now use LogRecord::attributes_iter()
  method to access them.
- Fixed various Metric aggregation bug related to
  ObservableCounter,UpDownCounter including
  [#1517](https://github.com/open-telemetry/opentelemetry-rust/issues/1517).
  [#2004](https://github.com/open-telemetry/opentelemetry-rust/pull/2004)
- Fixed a bug related to cumulative aggregation of `Gauge` measurements.
  [#1975](https://github.com/open-telemetry/opentelemetry-rust/issues/1975).
  [#2021](https://github.com/open-telemetry/opentelemetry-rust/pull/2021)
- Provide default implementation for `event_enabled` method in `LogProcessor`
  trait that returns `true` always.
- *Breaking* [#2041](https://github.com/open-telemetry/opentelemetry-rust/pull/2041)
  and [#2057](https://github.com/open-telemetry/opentelemetry-rust/pull/2057)
  - The Exporter::export() interface is modified as below:
    Previous Signature:

    ```rust
    async fn export<'a>(&mut self, batch: Vec<Cow<'a, LogData>>) -> LogResult<()>;
    ```

    Updated Signature:

    ```rust
    async fn export(&mut self, batch: LogBatch<'_>) -> LogResult<()>;
    ```

    where

    ```rust
    pub struct LogBatch<'a> {

      data: &'a [(&'a LogRecord, &'a InstrumentationLibrary)],
    }
    ```

    This change enhances performance by reducing unnecessary heap allocations and maintains object safety, allowing for more efficient handling of log records. It also simplifies the processing required by exporters. Exporters no longer need to determine if the LogData is borrowed or owned, as they now work directly with references. As a result, exporters must explicitly create a copy of LogRecord and/or InstrumentationLibrary when needed, as the new interface only provides references to these structures.

## v0.24.1

- Add hidden method to support tracing-opentelemetry

## v0.24.0

- Add "metrics", "logs" to default features. With this, default feature list is
  "trace", "metrics" and "logs".
- Add `with_resource` on Builder for LoggerProvider, replacing the `with_config`
  method. Instead of using
  `.with_config(Config::default().with_resource(RESOURCE::default()))` users
  must now use `.with_resource(RESOURCE::default())` to configure Resource on
  logger provider.
- Removed dependency on `ordered-float`.
- Removed `XrayIdGenerator`, which was marked deprecated since 0.21.3. Use
  [`opentelemetry-aws`](https://crates.io/crates/opentelemetry-aws), version
  0.10.0 or newer.
- Performance Improvement - Counter/UpDownCounter instruments internally use
  `RwLock` instead of `Mutex` to reduce contention

- *Breaking* [1726](https://github.com/open-telemetry/opentelemetry-rust/pull/1726)
  Update `LogProcessor::emit()` method to take mutable reference to LogData. This is breaking
  change for LogProcessor developers. If the processor needs to invoke the exporter
  asynchronously, it should clone the data to ensure it can be safely processed without
  lifetime issues. Any changes made to the log data before cloning in this method will be reflected in the next log processor in the chain, as well as to the exporter.
- *Breaking* [1726](https://github.com/open-telemetry/opentelemetry-rust/pull/1726)
 Update `LogExporter::export()` method to accept a batch of log data, which can be either a
 reference or owned`LogData`. If the exporter needs to process the log data
 asynchronously, it should clone the log data to ensure it can be safely processed without
 lifetime issues.
- Clean up public methods in SDK.
  - [`TracerProvider::span_processors`] and [`TracerProvider::config`] was removed as it's not part of the spec.
  - Added `non_exhaustive` annotation to [`trace::Config`]. Marked [`config`] as deprecated since it's only a wrapper for `Config::default`
  - Removed [`Tracer::tracer_provider`] and [`Tracer::instrument_libraries`] as it's not part of the spec.

- *Breaking* [#1830](https://github.com/open-telemetry/opentelemetry-rust/pull/1830/files) [Traces SDK] Improves
  performance by sending Resource information to processors (and exporters) once, instead of sending with every log. If you are an author
  of Processor, Exporter, the following are *BREAKING* changes.
  - Implement `set_resource` method in your custom SpanProcessor, which invokes exporter's `set_resource`.
  - Implement `set_resource` method in your custom SpanExporter. This method should save the resource object
      in original or serialized format, to be merged with every span event during export.
  - `SpanData` doesn't have the resource attributes. The `SpanExporter::export()` method needs to merge it
      with the earlier preserved resource before export.

- *Breaking* [1836](https://github.com/open-telemetry/opentelemetry-rust/pull/1836) `SpanProcessor::shutdown` now takes an immutable reference to self. Any reference can call shutdown on the processor. After the first call to `shutdown` the processor will not process any new spans.

- *Breaking* [1850] (<https://github.com/open-telemetry/opentelemetry-rust/pull/1850>) `LoggerProvider::log_processors()` and `LoggerProvider::resource()` are not public methods anymore. They are only used within the `opentelemetry-sdk` crate.

- [1857](https://github.com/open-telemetry/opentelemetry-rust/pull/1857) Fixed an issue in Metrics SDK which prevented export errors from being send to global error handler. With the fix, errors occurring during export like OTLP Endpoint unresponsive shows up in stderr by default.

- [1869](https://github.com/open-telemetry/opentelemetry-rust/pull/1869) Added a `target` field to `LogRecord` structure, populated by `opentelemetry-appender-tracing` and `opentelemetry-appender-log` appenders.

```rust
async fn export<'a>(&mut self, batch: Vec<Cow<'a, LogData>>) -> LogResult<()>;
```

where `LogRecord` within `LogData` now includes:

```rust
LogData {
  LogRecord {
    event_name,
    target,  // newly added
    timestamp,
    observed_timestamp,
    trace_context,
    trace_context,
    severity_number,
    body,
    attributes
  }
  Instrumentation {
    name,
    version,
    schema_url,
    version
  }
}
```

The `LogRecord::target` field contains the actual target/component emitting the logs, while the `Instrumentation::name` contains the name of the OpenTelemetry appender.

- *Breaking* [#1674](https://github.com/open-telemetry/opentelemetry-rust/pull/1674) Update to `http` v1 types (via `opentelemetry-http` update)
- Update `opentelemetry` dependency version to 0.24
- Update `opentelemetry-http` dependency version to 0.13

## v0.23.0

- Fix SimpleSpanProcessor to be consistent with log counterpart. Also removed
  dependency on crossbeam-channel.
  [1612](https://github.com/open-telemetry/opentelemetry-rust/pull/1612/files)
- [#1422](https://github.com/open-telemetry/opentelemetry-rust/pull/1422)
  Fix metrics aggregation bug when using Views to drop attributes.
- [#1766](https://github.com/open-telemetry/opentelemetry-rust/pull/1766)
  Fix Metrics PeriodicReader to trigger first collect/export at the first interval
  instead of doing it right away.
- [#1623](https://github.com/open-telemetry/opentelemetry-rust/pull/1623) Add Drop implementation for SdkMeterProvider,
  which shuts down `MetricReader`s, thereby allowing metrics still in memory to be flushed out.
- *Breaking* [#1624](https://github.com/open-telemetry/opentelemetry-rust/pull/1624) Remove `OsResourceDetector` and
  `ProcessResourceDetector` resource detectors, use the
  [`opentelemetry-resource-detector`](https://crates.io/crates/opentelemetry-resource-detectors) instead.
- [#1636](https://github.com/open-telemetry/opentelemetry-rust/pull/1636) [Logs SDK] Improves performance by sending
  Resource information to processors (and exporters) once, instead of sending with every log. If you are an author
  of Processor, Exporter, the following are *BREAKING* changes.
  - Implement `set_resource` method in your custom LogProcessor, which invokes exporter's `set_resource`.
  - Implement `set_resource` method in your custom LogExporter. This method should save the resource object
      in original or serialized format, to be merged with every log event during export.
  - `LogData` doesn't have the resource attributes. The `LogExporter::export()` method needs to merge it
      with the earlier preserved resource before export.
- Baggage propagation error will be reported to global error handler [#1640](https://github.com/open-telemetry/opentelemetry-rust/pull/1640)
- Improves `shutdown` behavior of `LoggerProvider` and `LogProcessor` [#1643](https://github.com/open-telemetry/opentelemetry-rust/pull/1643).
  - `shutdown` can be called by any clone of the `LoggerProvider` without the need of waiting on all `Logger` drops. Thus, `try_shutdown` has been removed.
  - `shutdown` methods in `LoggerProvider` and `LogProcessor` now takes a immutable reference
  - After `shutdown`, `LoggerProvider` will return noop `Logger`
  - After `shutdown`, `LogProcessor` will not process any new logs
- Moving LogRecord implementation to the SDK. [1702](https://github.com/open-telemetry/opentelemetry-rust/pull/1702).
  - Relocated `LogRecord` struct to SDK, as an implementation for the trait in the API.
- *Breaking* [#1729](https://github.com/open-telemetry/opentelemetry-rust/pull/1729)
  - Update the return type of `TracerProvider.span_processors()` from `&Vec<Box<dyn SpanProcessor>>` to `&[Box<dyn SpanProcessor>]`.
  - Update the return type of `LoggerProvider.log_processors()` from `&Vec<Box<dyn LogProcessor>>` to `&[Box<dyn LogProcessor>]`.
- Update `opentelemetry` dependency version to 0.23
- Update `opentelemetry-http` dependency version to 0.12
- *Breaking* [#1750](https://github.com/open-telemetry/opentelemetry-rust/pull/1729)
  - Update the return type of `LoggerProvider.shutdown()` from `Vec<LogResult<()>>` to `LogResult<()>`.

## v0.22.1

### Fixed

- [#1576](https://github.com/open-telemetry/opentelemetry-rust/pull/1576)
  Fix Span kind is always set to "internal".

## v0.22.0

### Deprecated

- XrayIdGenerator in the opentelemetry-sdk has been deprecated and moved to version 0.10.0 of the opentelemetry-aws crate.

### Added

- [#1410](https://github.com/open-telemetry/opentelemetry-rust/pull/1410) Add experimental synchronous gauge
- [#1471](https://github.com/open-telemetry/opentelemetry-rust/pull/1471) Configure batch log record processor via [`OTEL_BLRP_*`](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/configuration/sdk-environment-variables.md#batch-logrecord-processor) environment variables and via `OtlpLogPipeline::with_batch_config`
- [#1503](https://github.com/open-telemetry/opentelemetry-rust/pull/1503) Make the documentation for In-Memory exporters visible.

- [#1526](https://github.com/open-telemetry/opentelemetry-rust/pull/1526)
Performance Improvement : Creating Spans and LogRecords are now faster, by avoiding expensive cloning of `Resource` for every Span/LogRecord.

### Changed

- *Breaking*
[#1313](https://github.com/open-telemetry/opentelemetry-rust/pull/1313)
[#1350](https://github.com/open-telemetry/opentelemetry-rust/pull/1350)
  Changes how Span links/events are stored to achieve performance gains. See
  below for details:

  *Behavior Change*: When enforcing `max_links_per_span`, `max_events_per_span`
  from `SpanLimits`, links/events are kept in the first-come order. The previous
  "eviction" based approach is no longer performed.

  *Breaking Change Affecting Exporter authors*:

  `SpanData` now stores `links` as `SpanLinks` instead of `EvictedQueue` where
  `SpanLinks` is a struct with a `Vec` of links and `dropped_count`.

  `SpanData` now stores `events` as `SpanEvents` instead of `EvictedQueue` where
  `SpanEvents` is a struct with a `Vec` of events and `dropped_count`.
- *Breaking* Remove `TextMapCompositePropagator` [#1373](https://github.com/open-telemetry/opentelemetry-rust/pull/1373). Use `TextMapCompositePropagator` in opentelemetry API.

- [#1375](https://github.com/open-telemetry/opentelemetry-rust/pull/1375/) Fix metric collections during PeriodicReader shutdown
- *Breaking* [#1480](https://github.com/open-telemetry/opentelemetry-rust/pull/1480) Remove fine grained `BatchConfig` configurations from `BatchLogProcessorBuilder` and `BatchSpanProcessorBuilder`. Use `BatchConfigBuilder` to construct a `BatchConfig` instance and pass it using `BatchLogProcessorBuilder::with_batch_config` or `BatchSpanProcessorBuilder::with_batch_config`.
- *Breaking* [#1480](https://github.com/open-telemetry/opentelemetry-rust/pull/1480) Remove mutating functions from `BatchConfig`, use `BatchConfigBuilder` to construct a `BatchConfig` instance.
- *Breaking* [#1495](https://github.com/open-telemetry/opentelemetry-rust/pull/1495) Remove Batch LogRecord&Span Processor configuration via non-standard environment variables. Use the following table to migrate from the no longer supported non-standard environment variables to the standard ones.

| No longer supported             | Standard equivalent       |
|---------------------------------|---------------------------|
| OTEL_BLRP_SCHEDULE_DELAY_MILLIS | OTEL_BLRP_SCHEDULE_DELAY  |
| OTEL_BLRP_EXPORT_TIMEOUT_MILLIS | OTEL_BLRP_EXPORT_TIMEOUT  |
| OTEL_BSP_SCHEDULE_DELAY_MILLIS  | OTEL_BSP_SCHEDULE_DELAY   |
| OTEL_BSP_EXPORT_TIMEOUT_MILLIS  | OTEL_BSP_EXPORT_TIMEOUT   |

- *Breaking* [#1455](https://github.com/open-telemetry/opentelemetry-rust/pull/1455) Make the LoggerProvider Owned
  - `Logger` now takes an Owned Logger instead of a `Weak<LoggerProviderInner>`
  - `LoggerProviderInner` is no longer `pub (crate)`
  - `Logger.provider()` now returns `&LoggerProvider` instead of an `Option<LoggerProvider>`

- [#1519](https://github.com/open-telemetry/opentelemetry-rust/pull/1519) Performance improvements
    when calling `Counter::add()` and `UpDownCounter::add()` with an empty set of attributes
    (e.g. `counter.Add(5, &[])`)

- *Breaking* Renamed `MeterProvider` and `Meter` to `SdkMeterProvider` and `SdkMeter` respectively to avoid name collision with public API types. [#1328](https://github.com/open-telemetry/opentelemetry-rust/pull/1328)

### Fixed

- [#1481](https://github.com/open-telemetry/opentelemetry-rust/pull/1481) Fix error message caused by race condition when using PeriodicReader

## v0.21.2

### Fixed

- Fix delta aggregation metric reuse. [#1434](https://github.com/open-telemetry/opentelemetry-rust/pull/1434)
- Fix `max_scale` validation of exponential histogram configuration. [#1452](https://github.com/open-telemetry/opentelemetry-rust/pull/1452)

## v0.21.1

### Fixed

- Fix metric export corruption if gauges have not received a last value. [#1363](https://github.com/open-telemetry/opentelemetry-rust/pull/1363)
- Return consistent `Meter` for a given scope from `MeterProvider`. [#1351](https://github.com/open-telemetry/opentelemetry-rust/pull/1351)

## v0.21.0

### Added

- Log warning if two instruments have the same name with different [#1266](https://github.com/open-telemetry/opentelemetry-rust/pull/1266)
  casing
- Log warning if view is created with empty criteria [#1266](https://github.com/open-telemetry/opentelemetry-rust/pull/1266)
- Add exponential histogram support [#1267](https://github.com/open-telemetry/opentelemetry-rust/pull/1267)
- Add `opentelemetry::sdk::logs::config()` for parity with `opentelemetry::sdk::trace::config()` [#1197](https://github.com/open-telemetry/opentelemetry-rust/pull/1197)

### Changed

- Bump MSRV to 1.65 [#1318](https://github.com/open-telemetry/opentelemetry-rust/pull/1318)
- Default Resource (the one used when no other Resource is explicitly provided) now includes `TelemetryResourceDetector`,
  populating "telemetry.sdk.*" attributes.
  [#1194](https://github.com/open-telemetry/opentelemetry-rust/pull/1194).
- Bump MSRV to 1.64 [#1203](https://github.com/open-telemetry/opentelemetry-rust/pull/1203)
- Add unit/doc tests for MeterProvider [#1220](https://github.com/open-telemetry/opentelemetry-rust/pull/1220)
- Changed dependency from `opentelemetry_api` to `opentelemetry` as the latter
  is now the API crate. [#1226](https://github.com/open-telemetry/opentelemetry-rust/pull/1226)
- Add in memory span exporter [#1216](https://github.com/open-telemetry/opentelemetry-rust/pull/1216)
- Add in memory log exporter [#1231](https://github.com/open-telemetry/opentelemetry-rust/pull/1231)
- Add `Sync` bound to the `SpanExporter` and `LogExporter` traits [#1240](https://github.com/open-telemetry/opentelemetry-rust/pull/1240)
- Move `MetricsProducer` config to builders to match other config [#1266](https://github.com/open-telemetry/opentelemetry-rust/pull/1266)
- Return error earlier if readers are shut down [#1266](https://github.com/open-telemetry/opentelemetry-rust/pull/1266)
- Add `/` to valid characters for instrument names [#1269](https://github.com/open-telemetry/opentelemetry-rust/pull/1269)
- Increase instrument name maximum length from 63 to 255 [#1269](https://github.com/open-telemetry/opentelemetry-rust/pull/1269)
- Updated crate documentation and examples.
  [#1256](https://github.com/open-telemetry/opentelemetry-rust/issues/1256)
- Replace regex with glob [#1301](https://github.com/open-telemetry/opentelemetry-rust/pull/1301)
- *Breaking*
  [#1293](https://github.com/open-telemetry/opentelemetry-rust/issues/1293)
  makes few breaking changes with respect to how Span attributes are stored to
  achieve performance gains. See below for details:

  *Behavior Change*:

  SDK will no longer perform de-duplication of Span attribute Keys. Please share
  [feedback
  here](https://github.com/open-telemetry/opentelemetry-rust/issues/1300), if
  you are affected.

  *Breaking Change Affecting Exporter authors*:

   `SpanData` now stores `attributes` as `Vec<KeyValue>` instead of
  `EvictedHashMap`. `SpanData` now expose `dropped_attributes_count` as a
  separate field.

  *Breaking Change Affecting Sampler authors*:

  `should_sample` changes `attributes` from `OrderMap<Key, Value>` to
  `Vec<KeyValue>`.
- *Breaking* Move type argument from `RuntimeChannel<T>` to associated types [#1314](https://github.com/open-telemetry/opentelemetry-rust/pull/1314)

### Removed

- Remove context from Metric force_flush [#1245](https://github.com/open-telemetry/opentelemetry-rust/pull/1245)
- Remove `logs::BatchMessage` and `trace::BatchMessage` types [#1314](https://github.com/open-telemetry/opentelemetry-rust/pull/1314)

### Fixed

- Fix metric instrument name validation to include `_` [#1274](https://github.com/open-telemetry/opentelemetry-rust/pull/1274)

## v0.20.0

### Added

- Implement cardinality limits for metric streams
  [#1066](https://github.com/open-telemetry/opentelemetry-rust/pull/1066).
- Propagate shutdown calls from `PeriodicReader` to metrics exporter
  [#1138](https://github.com/open-telemetry/opentelemetry-rust/pull/1138).
- Add in memory metrics exporter #1017

### Changed

- New metrics SDK #1000
- Use `Cow<'static, str>` instead of `&'static str` #1018
- Unify trace and logs runtime extensions traits. #1067

### Changed

- Fix EvictedQueue bug when capacity is set to 0
  [#1151](https://github.com/open-telemetry/opentelemetry-rust/pull/1151).

### Removed

- Samplers no longer has access to `InstrumentationLibrary` as one of parameters
  to `should_sample`.
  [#1041](https://github.com/open-telemetry/opentelemetry-rust/pull/1041).
- Synchronous instruments no longer accepts `Context` while reporting
  measurements. [#1076](https://github.com/open-telemetry/opentelemetry-rust/pull/1076).
- Don't use CARGO_BIN_NAME for service name #1150

### Fixed

- Wait for exports on the simple span processor's ForceFlush #1030

## v0.19.0

### Added

- Add instrument validation to `InstrumentBuilder` [#884](https://github.com/open-telemetry/opentelemetry-rust/pull/884).
- Add `TelemetryResourceDetector` [#899](https://github.com/open-telemetry/opentelemetry-rust/pull/899).
- Add support for instrumentation scope attributes [#1021](https://github.com/open-telemetry/opentelemetry-rust/pull/1021).

### Changed

- Update to `opentelemetry_api` v0.19.
- Update to `opentelemetry_http` v0.8.
- Bump MSRV to 1.57 [#953](https://github.com/open-telemetry/opentelemetry-rust/pull/953).
- Fix doc in `ShouldSample` trait [#951](https://github.com/open-telemetry/opentelemetry-rust/pull/951)
- Only run `ParentBased` delegate sampler when there is no parent [#948](https://github.com/open-telemetry/opentelemetry-rust/pull/948).
- Improve `SdkProvidedResourceDetector`'s doc [#964](https://github.com/open-telemetry/opentelemetry-rust/pull/964).
- Update dependencies and bump MSRV to 1.60 [#969](https://github.com/open-telemetry/opentelemetry-rust/pull/969).
- Use CARGO_BIN_NAME as default service name [#991](https://github.com/open-telemetry/opentelemetry-rust/pull/991).

### Removed

- Remove `in_memory` settings [#946](https://github.com/open-telemetry/opentelemetry-rust/pull/946).

## main

### Changed

- Update the Number in the SDK API to support min and max. #989

## v0.18.0

### Changed

- *BREAKING* `struct`s which implement `ShouldSample` a.k.a Custom Samplers must now
  implement `Clone`. This enables (#833)
- SDK split from `opentelemetry` crate
