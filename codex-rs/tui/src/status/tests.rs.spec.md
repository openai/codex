## Overview
Regression tests for the `/status` card. They confirm that configuration metadata, rate-limit displays, truncation behavior, and context window usage are rendered correctly.

## Detailed Behavior
- Test scaffolding:
  - `test_config` loads a default config anchored to a temporary Codex home.
  - Helpers (`render_lines`, `sanitize_directory`, `reset_at_from`) normalize rendered output and timestamps for stable snapshots.
- Snapshot tests:
  - `status_snapshot_includes_reasoning_details` ensures reasoning effort/summary metadata, sandbox mode, approvals, rate limits, and directory formatting appear in the card.
  - `status_snapshot_includes_monthly_limit` validates monthly (long window) rate-limit rendering.
  - `status_snapshot_truncates_in_narrow_terminal` exercises narrow width rendering to confirm labels truncate cleanly.
  - `status_snapshot_shows_missing_limits_message` and `status_snapshot_shows_empty_limits_message` inspect fallback messaging when rate limits are absent/pending.
- Behavioral tests:
  - `status_card_token_usage_excludes_cached_tokens` asserts that cached tokens aren’t included in the displayed totals.
  - `status_context_window_uses_last_usage` verifies the context window line uses the immediate turn’s usage rather than cumulative totals.
  - Additional snapshot coverage ensures deduped directories and agent summaries remain stable across platforms (Windows path normalization).

## Broader Context
- Protects `status/card.rs` against regressions when formatting helpers or configuration handling changes.
- Relies on `rate_limit_snapshot_display` to convert protocol data into display-friendly structures before rendering.

## Technical Debt
- Snapshot sanitization replaces directory paths but not other environment-specific values; tests might still fail if new fields introduce absolute paths.
- Tests use fixed timestamps; adding edge cases for timezone transitions could further harden reset-time formatting.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Expand coverage for additional account types once new authentication flows are supported.
related_specs:
  - card.rs.spec.md
  - rate_limits.rs.spec.md
