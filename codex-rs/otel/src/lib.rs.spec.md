## Overview
`lib.rs` wires together the OTLP telemetry helpers. It re-exports configuration and event-manager modules and conditionally exposes the OTLP provider when the `otel` feature is enabled, falling back to a no-op stub otherwise.

## Detailed Behavior
- Public modules: `config`, `otel_event_manager` (always available) and `otel_provider` behind the `otel` feature flag.
- When `otel` is disabled, defines a stub `OtelProvider` whose `from` function always returns `None` and `headers` returns an empty map, keeping call sites free from feature guards.

## Broader Context
- Consumers call `OtelProvider::from` to decide whether to enable OTLP logging; the stub ensures binaries compile without the feature.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./otel_provider.rs.spec.md
  - ./otel_event_manager.rs.spec.md
