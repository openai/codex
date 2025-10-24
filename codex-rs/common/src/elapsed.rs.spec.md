## Overview
`common::elapsed` provides lightweight helpers for formatting elapsed time. When the `elapsed` feature is enabled, consumers can call `format_elapsed` and `format_duration` to convert timers into human-readable strings without re-implementing the formatting rules.

## Detailed Behavior
- `format_elapsed` accepts an `Instant`, computes the elapsed `Duration`, and delegates to `format_duration`.
- `format_duration` converts a `Duration` into milliseconds and forwards to the internal `format_elapsed_millis` helper to keep the decision logic focused on integer math.
- `format_elapsed_millis` applies tiered formatting:
  - Fewer than 1,000 ms render as `"Nms"`.
  - Between 1,000 ms and 59,999 ms render as seconds with two decimal places (e.g., `"1.50s"`).
  - 60,000 ms or more render as `"Mm SSs"`, zero-padding seconds and supporting multi-minute values.
- Unit tests cover each tier, including boundary conditions such as exactly 0 ms, 60 seconds, and one hour, ensuring stable formatting across updates.

## Broader Context
- Shared by CLI status lines and potential logging utilities to present compact timing information. Because the helper is feature-gated, binaries that do not display timings can omit the dependency to keep builds lean.
- The formatting intentionally avoids localization; consumer specs should mention if locale-aware representations become necessary.
- Context can't yet be determined for sub-millisecond precision requirements; revisit if profiling tools require finer granularity.

## Technical Debt
- None observed; formatting logic is simple, deterministic, and comprehensively tested.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
