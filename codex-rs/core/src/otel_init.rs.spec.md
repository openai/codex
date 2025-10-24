## Overview
`core::otel_init` wires Codex configuration into the `codex-otel` telemetry provider. It builds the exporter settings (HTTP/JSON, HTTP/binary, gRPC, or disabled) and exposes a tracing filter that limits exports to Codex-owned spans.

## Detailed Behavior
- `build_provider` inspects `Config::otel.exporter`:
  - `None` → `OtelExporter::None`.
  - `OtlpHttp` → clones endpoint, headers, protocol (`Json` vs `Binary`) into `OtelExporter::OtlpHttp`.
  - `OtlpGrpc` → clones endpoint/headers into `OtelExporter::OtlpGrpc`.
- The method populates `OtelSettings` with:
  - `service_name` from `default_client::originator`.
  - `service_version` passed by the caller (typically the crate version).
  - `codex_home` directory and deployment environment from config.
  - The computed exporter variant.
- Finally it calls `OtelProvider::from(&OtelSettings)`, returning `Ok(None)` when telemetry is disabled.
- `codex_export_filter` keeps only tracing metadata whose target starts with `codex_otel`, ensuring third-party dependencies do not leak into Codex telemetry streams.

## Broader Context
- CLI/TUI binaries call `build_provider` during startup to initialize global tracing subscribers. The returned `OtelProvider` implements shutdown hooks and batching policies defined in `codex-otel`.
- Exporter settings mirror the TOML schema exposed to users; documentation in `docs/code-specs` should reference this mapping when describing observability configuration.
- Context can't yet be determined for custom exporters (e.g., stdout); supporting additional variants would require expanding both config enums and this translation layer.

## Technical Debt
- `build_provider` clones header maps for every call; caching or memoizing the provider could avoid redundant allocations when multiple components initialize telemetry with the same config.
- The filter function is hard-coded; exposing a configurable allowlist could make it easier to debug non-Codex spans when needed.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Cache or reuse constructed `OtelProvider` instances when configuration is unchanged to reduce duplicate initialization work.
related_specs:
  - ./client.rs.spec.md
  - ./config.rs.spec.md
  - ../mod.spec.md
