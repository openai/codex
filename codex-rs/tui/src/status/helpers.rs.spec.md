## Overview
Utility functions that generate status card strings: model display metadata, agent summaries, account information, token formatting, directory shortening, timestamp formatting, and simple title casing.

## Detailed Behavior
- `compose_model_display` inspects config summary entries for “reasoning effort” and “reasoning summaries”, appending descriptors (e.g., “reasoning medium”, “summaries off”) to the base model name.
- `compose_agents_summary` enumerates project doc paths (`discover_project_doc_paths`), normalizes them relative to the workspace, and joins them with commas. Handles cases where docs sit above the CWD, using `..` prefixes, or returns `<none>` when missing.
- `compose_account_display` reads the auth JSON from `codex_home`; when ChatGPT tokens exist it returns `StatusAccountDisplay::ChatGpt` with email/plan (title-cased). If only an API key is present, returns `ApiKey`; otherwise `None`.
- Formatting helpers:
  - `format_tokens_compact` converts large token counts into compact strings (`1.2K`, `3.4M`, etc.) trimming trailing zeros.
  - `format_directory_display` renders the current directory relative to `~` when possible and center-truncates long paths using `text_formatting::center_truncate_path` when `max_width` is provided.
  - `format_reset_timestamp` prints local time (HH:MM) and, when needed, the day/month suffix.
  - `title_case` capitalizes the first letter of a string and lowercases the rest.

## Broader Context
- Consumed by `status/card.rs` to populate status lines and by rate-limit rendering to display reset times and compact counts.
- Integrates with authentication (`get_auth_file`, `try_read_auth_json`) and project documentation discovery to surface accurate metadata.

## Technical Debt
- `compose_agents_summary` assembles relative paths manually; leveraging `pathdiff` or a shared path utility could reduce edge cases.
- `compose_account_display` silently ignores errors reading auth files; surfacing diagnostics could aid troubleshooting.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Replace manual relative-path computation in `compose_agents_summary` with shared helpers to reduce duplication.
related_specs:
  - card.rs.spec.md
  - rate_limits.rs.spec.md
