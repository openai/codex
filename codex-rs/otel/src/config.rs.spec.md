## Overview
`config.rs` defines the configuration structures for Codex’s OpenTelemetry logging. It describes the service metadata and exporter variants supported by the Otel provider.

## Detailed Behavior
- `OtelSettings` captures environment, service name/version, Codex home path, and selected exporter.
- `OtelExporter` enum allows three states:
  - `None` (telemetry disabled),
  - `OtlpGrpc` with endpoint plus headers,
  - `OtlpHttp` with endpoint, headers, and protocol (`Binary` or `Json`).
- `OtelHttpProtocol` distinguishes HTTP binary vs. JSON payloads for OTLP exporters.

## Broader Context
- Used by configuration loaders to instantiate `OtelProvider` with the correct exporter settings.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./otel_provider.rs.spec.md
