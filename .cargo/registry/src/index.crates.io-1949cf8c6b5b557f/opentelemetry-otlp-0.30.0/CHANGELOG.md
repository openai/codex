# Changelog

## vNext

## 0.30.0

Released 2025-May-23

- Update `opentelemetry` dependency version to 0.30
- Update `opentelemetry_sdk` dependency version to 0.30
- Update `opentelemetry-http` dependency version to 0.30
- Update `opentelemetry-proto` dependency version to 0.30
- Update `tonic` dependency version to 0.13
- Re-export `tonic` types under `tonic_types`
  [2898](https://github.com/open-telemetry/opentelemetry-rust/pull/2898)
- Publicly re-exported `MetricExporterBuilder`, `SpanExporterBuilder`, and
  `LogExporterBuilder` types, enabling users to directly reference and use these
  builder types for metrics, traces, and logs exporters.
  [2966](https://github.com/open-telemetry/opentelemetry-rust/pull/2966)

## 0.29.0

Released 2025-Mar-21

- Update `opentelemetry` dependency version to 0.29
- Update `opentelemetry_sdk` dependency version to 0.29
- Update `opentelemetry-http` dependency version to 0.29
- Update `opentelemetry-proto` dependency version to 0.29

- The `OTEL_EXPORTER_OTLP_TIMEOUT`, `OTEL_EXPORTER_OTLP_TRACES_TIMEOUT`, `OTEL_EXPORTER_OTLP_METRICS_TIMEOUT` and `OTEL_EXPORTER_OTLP_LOGS_TIMEOUT` are changed from seconds to miliseconds.
- Fixed `.with_headers()` in `HttpExporterBuilder` to correctly support multiple key/value pairs. [#2699](https://github.com/open-telemetry/opentelemetry-rust/pull/2699)
- Fixed
  [#2770](https://github.com/open-telemetry/opentelemetry-rust/issues/2770)
  partially to properly handle `shutdown()` when using `http`. (`tonic` still
  does not do proper shutdown)
- *Breaking*
 ExporterBuilder's build() method now Result with `ExporterBuildError` being the
 Error variant. Previously it returned signal specific errors like `LogError`
 from the `opentelemetry_sdk`, which are no longer part of the sdk. No changes
 required if you were using unwrap/expect. If you were matching on the returning
 Error enum, replace with the enum `ExporterBuildError`. Unlike the previous
 `Error` which contained many variants unrelated to building an exporter, the
 new one returns specific variants applicable to building an exporter. Some
 variants might be applicable only on select features.
 Also, now unused `Error` enum is removed.
- **Breaking** `ExportConfig`'s `timeout` field is now optional(`Option<Duration>`)
- **Breaking** Export configuration done via code is final. ENV variables cannot be used to override the code config.
  Do not use code based config, if there is desire to control the settings via ENV variables.
  List of ENV variables and corresponding setting being affected by this change.
  - `OTEL_EXPORTER_OTLP_ENDPOINT` -> `ExportConfig.endpoint`
  - `OTEL_EXPORTER_OTLP_TIMEOUT` -> `ExportConfig.timeout`

## 0.28.0

Released 2025-Feb-10

- Update `opentelemetry` dependency version to 0.28.
- Update `opentelemetry_sdk` dependency version to 0.28.
- Update `opentelemetry-http` dependency version to 0.28.
- Update `opentelemetry-proto` dependency version to 0.28.
- Bump msrv to 1.75.0.
- Feature flag "populate-logs-event-name" is removed as no longer relevant.
  LogRecord's `event_name()` is now automatically populated on the newly added
  "event_name" field in LogRecord proto definition.
- Remove "grpc-tonic" feature from default, and instead add "http-proto" and
  "reqwest-blocking-client" features as default, to align with the
  specification.
  [2516](https://github.com/open-telemetry/opentelemetry-rust/pull/2516)
- Remove unnecessarily public trait `opentelemetry_otlp::metrics::MetricsClient`
  and `MetricExporter::new(..)` method. Use
  `MetricExporter::builder()...build()` to obtain `MetricExporter`.
- The HTTP clients (reqwest, reqwest-blocking, hyper) now support the
  export timeout interval configured in below order
  - Signal specific env variable `OTEL_EXPORTER_OTLP_TRACES_TIMEOUT`,
  `OTEL_EXPORTER_OTLP_LOGS_TIMEOUT` or `OTEL_EXPORTER_OTLP_TIMEOUT`.
  - `OTEL_EXPORTER_OTLP_TIMEOUT` env variable.
  - `with_http().with_timeout()` API method of
`LogExporterBuilder` and `SpanExporterBuilder` and `MetricsExporterBuilder`.
  - The default interval of 10 seconds is used if none is configured.

## 0.27.0

Released 2024-Nov-11

- Update `opentelemetry` dependency version to 0.27
- Update `opentelemetry_sdk` dependency version to 0.27
- Update `opentelemetry-http` dependency version to 0.27
- Update `opentelemetry-proto` dependency version to 0.27

- **BREAKING**:
  - ([#2217](https://github.com/open-telemetry/opentelemetry-rust/pull/2217)) **Replaced**: The `MetricsExporterBuilder` interface is modified from `with_temporality_selector` to `with_temporality` example can be seen below:
    Previous Signature:
    ```rust
    MetricsExporterBuilder::default().with_temporality_selector(DeltaTemporalitySelector::new())
    ```
    Updated Signature:
    ```rust
    MetricsExporterBuilder::default().with_temporality(opentelemetry_sdk::metrics::Temporality::Delta)
    ```
  - ([#2221](https://github.com/open-telemetry/opentelemetry-rust/pull/2221)) **Replaced**:
    - The `opentelemetry_otlp::new_pipeline().{trace,logging,metrics}()` interface is now replaced with `{TracerProvider,SdkMeterProvider,LoggerProvider}::builder()`.
    - The `opentelemetry_otlp::new_exporter()` interface is now replaced with `{SpanExporter,MetricsExporter,LogExporter}::builder()`.

    Pull request [#2221](https://github.com/open-telemetry/opentelemetry-rust/pull/2221) has a detailed migration guide in the description. See example below,
    and [basic-otlp](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-otlp/examples/basic-otlp/src/main.rs) for more details:

    Previous Signature:
    ```rust
    let logger_provider: LoggerProvider = opentelemetry_otlp::new_pipeline()
      .logging()
      .with_resource(RESOURCE.clone())
      .with_exporter(
          opentelemetry_otlp::new_exporter()
              .tonic()
              .with_endpoint("http://localhost:4317")
      )
      .install_batch(runtime::Tokio)?;
    ```
    Updated Signature:
    ```rust
    let exporter = LogExporter::builder()
        .with_tonic()
        .with_endpoint("http://localhost:4317")
        .build()?;

    Ok(LoggerProvider::builder()
        .with_resource(RESOURCE.clone())
        .with_batch_exporter(exporter, runtime::Tokio)
        .build())
    ```
  - **Renamed**
    - ([#2255](https://github.com/open-telemetry/opentelemetry-rust/pull/2255)): de-pluralize Metric types.
      - `MetricsExporter` -> `MetricExporter`
      - `MetricsExporterBuilder` -> `MetricExporterBuilder`

  - [#2263](https://github.com/open-telemetry/opentelemetry-rust/pull/2263)
  Support `hyper` client for opentelemetry-otlp. This can be enabled using flag `hyper-client`.
  Refer example: https://github.com/open-telemetry/opentelemetry-rust/tree/main/opentelemetry-otlp/examples/basic-otlp-http

## v0.26.0
Released 2024-Sep-30

- Update `opentelemetry` dependency version to 0.26
- Update `opentelemetry_sdk` dependency version to 0.26
- Update `opentelemetry-http` dependency version to 0.26
- Update `opentelemetry-proto` dependency version to 0.26
- Bump MSRV to 1.71.1 [2140](https://github.com/open-telemetry/opentelemetry-rust/pull/2140)

## v0.25.0

- Update `opentelemetry` dependency version to 0.25
- Update `opentelemetry_sdk` dependency version to 0.25
- Update `opentelemetry-http` dependency version to 0.25
- Update `opentelemetry-proto` dependency version to 0.25
- Starting with this version, this crate will align with `opentelemetry` crate
  on major,minor versions.
- **Breaking**
The logrecord event-name is added as an attribute only if the feature flag
`populate-logs-event-name` is enabled. The name of the attribute is changed from
"name" to "event.name".
[1994](https://github.com/open-telemetry/opentelemetry-rust/pull/1994),
[2050](https://github.com/open-telemetry/opentelemetry-rust/pull/2050)

## v0.17.0

- Add "metrics", "logs" to default features. With this, default feature list is
  "trace", "metrics" and "logs".
- **Breaking** `OtlpMetricPipeline.build()` no longer invoke the
  `global::set_meter_provider`. User who setup the pipeline must do it
  themselves using `global::set_meter_provider(meter_provider.clone());`.
- Add `with_resource` on `OtlpLogPipeline`, replacing the `with_config` method.
Instead of using
`.with_config(Config::default().with_resource(RESOURCE::default()))` users must
now use `.with_resource(RESOURCE::default())` to configure Resource when using
`OtlpLogPipeline`.
- **Breaking** The methods `OtlpTracePipeline::install_simple()` and `OtlpTracePipeline::install_batch()` would now return `TracerProvider` instead of `Tracer`.
  These methods would also no longer set the global tracer provider. It would now be the responsibility of users to set it by calling `global::set_tracer_provider(tracer_provider.clone());`. Refer to the [basic-otlp](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-otlp/examples/basic-otlp/src/main.rs) and [basic-otlp-http](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-otlp/examples/basic-otlp-http/src/main.rs) examples on how to initialize OTLP Trace Exporter.
- **Breaking** Correct the misspelling of "webkpi" to "webpki" in features [#1842](https://github.com/open-telemetry/opentelemetry-rust/pull/1842)
- Bump MSRV to 1.70 [#1840](https://github.com/open-telemetry/opentelemetry-rust/pull/1840)
- Fixing the OTLP HTTP/JSON exporter. [#1882](https://github.com/open-telemetry/opentelemetry-rust/pull/1882) - The exporter was broken in the
  previous release.
- **Breaking** [1869](https://github.com/open-telemetry/opentelemetry-rust/pull/1869) The OTLP logs exporter now overrides the [InstrumentationScope::name](https://github.com/open-telemetry/opentelemetry-proto/blob/b3060d2104df364136d75a35779e6bd48bac449a/opentelemetry/proto/common/v1/common.proto#L73) field with the `target` from `LogRecord`, if target is populated.
- Groups batch of `LogRecord` and `Span` by their resource and instrumentation scope before exporting, for better efficiency [#1873](https://github.com/open-telemetry/opentelemetry-rust/pull/1873).
- **Breaking** Update to `http` v1 and `tonic` v0.12 [#1674](https://github.com/open-telemetry/opentelemetry-rust/pull/1674)
- Update `opentelemetry` dependency version to 0.24
- Update `opentelemetry_sdk` dependency version to 0.24
- Update `opentelemetry-http` dependency version to 0.13
- Update `opentelemetry-proto` dependency version to 0.7

## v0.16.0

### Fixed

- URL encoded values in `OTEL_EXPORTER_OTLP_HEADERS` are now correctly decoded. [#1578](https://github.com/open-telemetry/opentelemetry-rust/pull/1578)
- OTLP exporter will not change the URL added through `ExportConfig` [#1706](https://github.com/open-telemetry/opentelemetry-rust/pull/1706)
- Default grpc endpoint will not have path based on signal(e.g `/v1/traces`) [#1706](https://github.com/open-telemetry/opentelemetry-rust/pull/1706)
- Fix feature flags for `OTEL_EXPORTER_OTLP_PROTOCOL_DEFAULT` [#1746](https://github.com/open-telemetry/opentelemetry-rust/pull/1746)

### Added

- Added `DeltaTemporalitySelector` ([#1568])
- Add `webkpi-roots` features to `reqwest` and `tonic` backends

[#1568]: https://github.com/open-telemetry/opentelemetry-rust/pull/1568

### Changed
 - **Breaking** Remove global provider for Logs [#1691](https://github.com/open-telemetry/opentelemetry-rust/pull/1691/)
      - The method OtlpLogPipeline::install_simple() and OtlpLogPipeline::install_batch() now return `LoggerProvider` instead of
      `Logger`. Refer to the [basic-otlp](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-otlp/examples/basic-otlp/src/main.rs) and [basic-otlp-http](https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-otlp/examples/basic-otlp-http/src/main.rs) examples for how to initialize OTLP Log Exporter to use with OpenTelemetryLogBridge and OpenTelemetryTracingBridge respectively.
- Update `opentelemetry` dependency version to 0.23
- Update `opentelemetry_sdk` dependency version to 0.23
- Update `opentelemetry-http` dependency version to 0.12
- Update `opentelemetry-proto` dependency version to 0.6

## v0.15.0

### Added

- Support custom channels in topic exporters  [#1335](https://github.com/open-telemetry/opentelemetry-rust/pull/1335)
- Allow specifying OTLP Tonic metadata from env variable [#1377](https://github.com/open-telemetry/opentelemetry-rust/pull/1377)

### Changed
- Update to tonic 0.11 and prost 0.12 [#1536](https://github.com/open-telemetry/opentelemetry-rust/pull/1536)

### Fixed
- Fix `tonic()` to the use correct port. [#1556](https://github.com/open-telemetry/opentelemetry-rust/pull/1556)

### Removed
- **Breaking** Remove support for surf HTTP client [#1537](https://github.com/open-telemetry/opentelemetry-rust/pull/1537)
- **Breaking** Remove support for grpcio transport [#1534](https://github.com/open-telemetry/opentelemetry-rust/pull/1534)

## v0.14.0

### Added

- Add `build_{signal}_exporter` methods to client builders [#1187](https://github.com/open-telemetry/opentelemetry-rust/pull/1187)
- Add `grpcio` metrics exporter [#1202](https://github.com/open-telemetry/opentelemetry-rust/pull/1202)
- Allow specifying OTLP HTTP headers from env variable [#1290](https://github.com/open-telemetry/opentelemetry-rust/pull/1290)

### Changed

- Bump MSRV to 1.65 [#1318](https://github.com/open-telemetry/opentelemetry-rust/pull/1318)
- Bump MSRV to 1.64 [#1203](https://github.com/open-telemetry/opentelemetry-rust/pull/1203)
- Changed dependency from `opentelemetry_api` to `opentelemetry` as the latter
  is now the API crate. [#1226](https://github.com/open-telemetry/opentelemetry-rust/pull/1226)
- Make `NoExporterBuilder` a compiling time error [#1272](https://github.com/open-telemetry/opentelemetry-rust/pull/1272)

## v0.13.0

### Added

- Add OTLP HTTP Metrics Exporter [#1020](https://github.com/open-telemetry/opentelemetry-rust/pull/1020).
- Add tonic compression support [#1165](https://github.com/open-telemetry/opentelemetry-rust/pull/1165).

### Changed

- make the tonic/transport feature optional #985
- update to opentelemetry-api v0.20.0

### Fixed

- Fix a missing import when http-proto is enabled without grpc-sys #1081

## v0.12.0

### Added

- Add batch config for otlp pipeline [#979](https://github.com/open-telemetry/opentelemetry-rust/pull/979).
- Add tonic interceptor [#901](https://github.com/open-telemetry/opentelemetry-rust/pull/901).

### Changed

- Update `opentelemetry` to 0.19.
- Update `opentelemetry-semantic-conventions` to 0.11.
- Update `opentelemetry-http` to 0.8.
- Bump MSRV to 1.57 [#953](https://github.com/open-telemetry/opentelemetry-rust/pull/953).
- Add `User-Agent` header on all exporters [#896](https://github.com/open-telemetry/opentelemetry-rust/pull/896).
- Improve OTLP exporter environment variable handling [#912](https://github.com/open-telemetry/opentelemetry-rust/pull/912).
- Fix the issue where tonic exporter builder ignored provided metadata [#937](https://github.com/open-telemetry/opentelemetry-rust/pull/937).
- Export `MetricsExporterBuilder` [#943](https://github.com/open-telemetry/opentelemetry-rust/pull/943).
- Report OTLP http export errors [#945](https://github.com/open-telemetry/opentelemetry-rust/pull/945).
- Change to export using v0.19.0 protobuf definitions. [#989](https://github.com/open-telemetry/opentelemetry-rust/pull/989).
- Update dependencies and bump MSRV to 1.60 [#969](https://github.com/open-telemetry/opentelemetry-rust/pull/969).


## v0.11.0

### Changed

- reduce `tokio` feature requirements #750
- Update to opentelemetry v0.18.0
- Update to opentelemetry-http v0.7.0
- Update `tonic` to 0.7 #783
- Automatically add traces / metrics paths #806

## v0.10.0

### Changed

- Update to opentelemetry v0.17.0
- Update to opentelemetry-http v0.6.0
- Update `tonic` to 0.6 #660

## v0.9.0

### Changed

- Merge metrics and tracing pipeline #585
- Update to opentelemetry v0.16.0

### Fixed

- `MetricsExporterBuilder` drops `exporter_pipeline` #590
- Improve error messages #603
- Upgrade `tonic` to `0.5.x` #597

## v0.8.0

### Changed

- Update grpcio version and add the coverage badge #556
- Update to opentelemetry v0.15.0

## v0.7.0

### Added

- adding otlp http transport, using proto binary #516

### Fixed

- docs cannot compile #507
- exporter cannot merge IntSum correctly. #518
- update metrics proto and metric transformation #535

### Changed

- Allow users to bring their own tonic channel  #515
- Remove default surf features #546
- Update to opentelemetry v0.14.0

### v0.6.0

### Added
- Examples on how to connect to an external otlp using tonic, tls and tokio #449
- Examples on how to connect to an external otlp using grpcio and tls #450
- `with_env` method for `OtlpPipelineBuilder` to use environment variables to config otlp pipeline #451
- Update `tracing-grpc` example to include extractors and injectors #464
- Mentioned `service.name` resource in README #476

### Changed
- Update to opentelemetry v0.13.0
- Update `tonic-build` dependency to 0.4 #463
- Update the opentelemetry pipeline to use API to choose grpc layer instead of feature #467
- Rename trace config with_default_sampler to with_sampler #482

### Removed
- Removed `from_env` and use environment variables to initialize the configurations by default #459
- Removed support for running tonic without tokio runtime #483

## v0.5.0

### Added

- Otlp metric exporter #402
- Otlp exporter integration test #424

### Changed

- Update to opentelemetry v0.12.0
- Update tokio to v1 #421

## v0.4.0

### Added

- Tonic support #352
- Add openssl feature flags for grpcio #367

### Changed

- Update to opentelemetry v0.11.0
- Update default otlp port to `4317` #388

### Fixed

- Propagate `Resource` information #366
- Propagate `Resource` in tonic as well #390

## v0.3.0

### Changed

- Update to opentelemetry v0.10.0

## v0.2.0

### Changed

- Update to opentelemetry v0.9.0
- Add exporter pipeline #210

## v0.1.0

### Added

- Initial Alpha implementation
