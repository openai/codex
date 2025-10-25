## Overview
`otel_provider.rs` constructs the OTLP logger provider when telemetry is enabled. It configures resources, selects the exporter (gRPC or HTTP), applies headers, and exposes a shutdown hook.

## Detailed Behavior
- `OtelProvider::from`:
  - Builds an `opentelemetry_sdk::Resource` with service name, version, and environment attributes.
  - Depending on `OtelExporter`, sets up either:
    - gRPC exporter with tonic transport and metadata (`HeaderMap` ➝ `MetadataMap`), or
    - HTTP exporter with binary/JSON protocol and headers.
  - Returns `None` when exporter is `None`, enabling callers to skip setup.
- `OtelProvider` wraps `SdkLoggerProvider`; `shutdown` and `Drop` call `logger.shutdown()` to flush telemetry.
- Utility functions parse header maps, logging decisions via `tracing::debug`.

## Broader Context
- Invoked by applications that opt into telemetry to configure an OTLP exporter compatible with their environment.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./config.rs.spec.md
