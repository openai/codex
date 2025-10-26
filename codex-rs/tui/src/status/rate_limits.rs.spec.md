## Overview
Transforms rate-limit telemetry into display rows for the `/status` card and renders progress bars summarizing usage. Handles both primary and secondary windows (e.g., short-term vs weekly limits) and applies user-friendly labels.

## Detailed Behavior
- Data structures:
  - `StatusRateLimitRow` holds the label (`"5h limit"`), percent used, and optional reset timestamp string.
  - `StatusRateLimitData` is either `Available(Vec<StatusRateLimitRow>)` or `Missing`.
  - `RateLimitWindowDisplay` tracks per-window percent used, reset time, and window length in minutes; `from_window` converts a `RateLimitWindow` into local time strings using `format_reset_timestamp`.
  - `RateLimitSnapshotDisplay` keeps optional primary and secondary window displays.
- Conversion functions:
  - `rate_limit_snapshot_display` wraps `RateLimitSnapshot`, converting timestamps into `Local` and storing `RateLimitWindowDisplay` values.
  - `compose_rate_limit_data` builds the list of `StatusRateLimitRow`s, using `get_limits_duration` to format window sizes (defaults to `"5h"` or `"weekly"` when metadata is absent) and capitalizing the first letter.
- Rendering helpers:
  - `render_status_limit_progress_bar` creates a fixed-width bar (20 segments) with filled (`█`) and empty (`░`) blocks based on clamped usage ratio.
  - `format_status_limit_summary` prints a rounded percentage string (`"75% used"`).

## Broader Context
- `status/card.rs` consumes these helpers to display rate-limit sections under the `/status` output, including inline progress bars and reset notices.
- The same display structures can be reused elsewhere when a quick textual snapshot of limits is required.

## Technical Debt
- Progress bar uses block characters that assume UTF-8 and terminals with block glyph support; fallbacks might be needed for minimal terminals.
- Labels default to `"5h"`/`"weekly"` when metadata is missing; providing explicit context (e.g., "short-term") could be clearer.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Add ASCII-friendly fallback for progress bars in environments lacking Unicode block glyphs.
related_specs:
  - card.rs.spec.md
  - helpers.rs.spec.md
