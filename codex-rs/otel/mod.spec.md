## Overview
`codex-otel` encapsulates Codex’s OpenTelemetry logging support. It defines configuration structures, optional OTLP exporters, and event helpers used to emit structured telemetry.

## Detailed Behavior
- `src/lib.rs` re-exports configuration, the event manager, and (when `otel` feature is enabled) the OTLP provider implementation.
- `src/config.rs` describes configurable OTLP exporters and service metadata.
- `src/otel_provider.rs` initializes an OTLP logger provider and handles shutdown when telemetry is enabled.
- `src/otel_event_manager.rs` emits structured events for Codex API/SSE interactions.

## Broader Context
- Consumers include the Codex CLI/TUI and backend components that wish to emit telemetry conditioned on configuration settings.

## Technical Debt
- None noted.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/config.rs.spec.md
  - ./src/otel_provider.rs.spec.md
  - ./src/otel_event_manager.rs.spec.md
