# Changelog

## vNext

## 0.30.1

Released 2025-June-05

- Bump `tracing-opentelemetry` to 0.31

## 0.30.0

Released 2025-May-23

- Updated `opentelemetry` dependency to version 0.30.0.


## 0.29.1

Released 2025-Mar-24

- Bump `tracing-opentelemetry` to 0.30


## 0.29.0

Released 2025-Mar-21

Fixes [1682](https://github.com/open-telemetry/opentelemetry-rust/issues/1682).
"spec_unstable_logs_enabled" feature now do not suppress logs for other layers.

The special treatment of the "message" field has been extended when recording
string values. With this change, when a log is emitted with a field named
"message" (and string value), its value is directly assigned to the LogRecord’s
body rather than being stored as an attribute named "message". This offers a
slight performance improvement over previous.

For example, the below will now produce LogRecord with the message value
populated as LogRecord's body:

```rust
error!(name: "my-event-name", target: "my-system", event_id = 20, user_name = "otel", user_email = "otel@opentelemetry.io", message = "This is an example message");
```

Previously, Body was only populated when the below style was used.

```rust
error!(name: "my-event-name", target: "my-system", event_id = 20, user_name = "otel", user_email = "otel@opentelemetry.io", "This is an example message");
```

This style, while slightly slower, should still be used when the value is not a
simple string, but require format arguments as in the below example.

```rust
error!(name: "my-event-name", target: "my-system", event_id = 20, user_name = "otel", user_email = "otel@opentelemetry.io", "This is an example message with format arguments {} and {}", "foo", "bar");
```

Fixes [2658](https://github.com/open-telemetry/opentelemetry-rust/issues/2658)
InstrumentationScope(Logger) used by the appender now uses an empty ("") named
Logger. Previously, a Logger with name and version of the crate was used.
Receivers (processors, exporters) are expected to use `LogRecord.target()` as
scope name. This is already done in OTLP Exporters, so this change should be
transparent to most users.

- Passes event name  to the `event_enabled` method on the `Logger`. This allows
  implementations (SDK, processor, exporters) to leverage this additional
  information to determine if an event is enabled.

- `u64`, `i128`, `u128` and `usize` values are stored as `opentelemetry::logs::AnyValue::Int`
when conversion is feasible. Otherwise stored as
`opentelemetry::logs::AnyValue::String`. This avoids unnecessary string
allocation when values can be represented in their original types.
- Byte arrays are stored as `opentelemetry::logs::AnyValue::Bytes` instead
of string.
- `Error` fields are reported using attribute named "exception.message". For
  example, the below will now report an attribute named "exception.message",
  instead of previously reporting the user provided attribute "error".
  `error!(....error = &OTelSdkError::AlreadyShutdown as &dyn std::error::Error...)`
- perf - small perf improvement by avoiding string allocation of `target`
- Update `opentelemetry` dependency version to 0.29.

## 0.28.1

Released 2025-Feb-12

- New *experimental* feature to use trace_id & span_id from spans created through the [tracing](https://crates.io/crates/tracing) crate (experimental_use_tracing_span_context) [#2438](https://github.com/open-telemetry/opentelemetry-rust/pull/2438)

## 0.28.0

Released 2025-Feb-10

- Update `opentelemetry` dependency version to 0.28.
- Bump msrv to 1.75.0.

## 0.27.0

Released 2024-Nov-11

- Update `opentelemetry` dependency version to 0.27

- Bump MSRV to 1.70 [#2179](https://github.com/open-telemetry/opentelemetry-rust/pull/2179)
- **Breaking** [2291](https://github.com/open-telemetry/opentelemetry-rust/pull/2291) Rename `logs_level_enabled flag` to `spec_unstable_logs_enabled`. Please enable this updated flag if the feature is needed. This flag will be removed once the feature is stabilized in the specifications.

## v0.26.0

Released 2024-Sep-30

- Update `opentelemetry` dependency version to 0.26
- [2101](https://github.com/open-telemetry/opentelemetry-rust/pull/2101) The `log` events emitted via the `tracing` pipeline using the `log-tracing` crate no longer include the target metadata as attributes. Exporters or backends that rely on this attribute should now access the target directly from the `LogRecord::target` field.

## v0.25.0

- Update `opentelemetry` dependency version to 0.25
- Starting with this version, this crate will align with `opentelemetry` crate
  on major,minor versions.
- Reduce heap allocation by using `&'static str` for `SeverityText`.

## v0.5.0

- [1869](https://github.com/open-telemetry/opentelemetry-rust/pull/1869) Utilize the `LogRecord::set_target()` method to pass the tracing target to the SDK.
  Exporters might use the target to override the instrumentation scope, which previously contained "opentelemetry-appender-tracing".

- **Breaking** [1928](https://github.com/open-telemetry/opentelemetry-rust/pull/1928) Insert tracing event name into LogRecord::event_name instead of attributes.
  - If using a custom exporter, then they must serialize this field directly from LogRecord::event_name instead of iterating over the attributes. OTLP Exporter is modified to handle this.
- Update `opentelemetry` dependency version to 0.24

## v0.4.0

- Removed unwanted dependency on opentelemetry-sdk.
- Update `opentelemetry` dependency version to 0.23

## v0.3.0

### Added

- New experimental metadata attributes feature (experimental\_metadata\_attributes) [#1380](https://github.com/open-telemetry/opentelemetry-rust/pull/1380)
  - Experimental new attributes for tracing metadata
  - Fixes the following for events emitted using log crate
    - Normalized metadata fields
    - Remove redundant metadata

## v0.2.0

### Changed

- Bump MSRV to 1.65 [#1318](https://github.com/open-telemetry/opentelemetry-rust/pull/1318)

### Added

- Add log appender versions to loggers (#1182)

## v0.1.0

Initial crate release
