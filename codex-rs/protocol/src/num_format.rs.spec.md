## Overview
`protocol::num_format` formats integer counts (typically tokens) for user-facing displays. It wraps ICU decimal formatting to provide locale-aware separators and implements base-10 SI suffix formatting for compact summaries.

## Detailed Behavior
- Initializes a shared `DecimalFormatter` via `OnceLock`, preferring the system locale when available and falling back to `"en-US"` as a safe default. This avoids repeated ICU setup across calls.
- `format_with_separators` formats an `i64` using the cached formatter, yielding strings like `"12,345"` or locale-specific equivalents.
- `format_si_suffix` and its helper `format_si_suffix_with_formatter` express counts using three significant figures and suffixes (`K`, `M`, `G`), scaling values down while preserving readability. Numbers beyond the `G` range fall back to whole-G precision with thousands separators.
- The helper clamps negative inputs to zero, ensuring token counts (which should be non-negative) never render with a minus sign.
- Unit tests cover key formatting edges, validating locale fallbacks and rounding boundaries for each suffix tier.

## Broader Context
- Used by token usage displays and telemetry summaries across the CLI and TUI. Because ICU formatting depends on the host locale, downstream specs should note that outputs may vary between environments.
- The module currently supports up to gigascale suffixes; clients needing larger units (e.g., teratokens) would require extending the suffix list.
- Context can't yet be determined for decimal precision customization or localization beyond default ICU behavior; consumers can extend the helper if such requirements emerge.

## Technical Debt
- None observed; the formatter encapsulates ICU usage cleanly and exercises key paths through tests.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./protocol.rs.spec.md
