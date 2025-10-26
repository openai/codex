## Overview
Builds the `/status` history card showing model configuration, directory, approvals, sandbox policy, agent summary, account info, token usage, and rate limits. The card is rendered as a bordered history cell combining the command echo (`/status`) and a formatted summary.

## Detailed Behavior
- `new_status_output` creates a `CompositeHistoryCell` with two parts:
  1. A `PlainHistoryCell` containing the `/status` command in magenta.
  2. A `StatusHistoryCell` populated from `Config`, usage stats, optional session id, and rate-limit snapshots.
- `StatusHistoryCell::new`:
  - Pulls configuration entries via `create_config_summary_entries` and `compose_model_display`.
  - Extracts approval mode, sandbox policy string, agent summary, and account details (`compose_account_display`).
  - Captures session id if available, and builds `StatusTokenUsageData`: total, input, output, and optional context window metrics based on `model_context_window` plus supplied `context_usage`.
  - Converts `RateLimitSnapshotDisplay` into `StatusRateLimitData` using `compose_rate_limit_data`.
- Rendering helpers:
  - `token_usage_spans` produces a compact summary (`total (input + output)`).
  - `context_window_spans` shows percent remaining and usage breakdown when context data exists.
  - `rate_limit_lines` renders progress bars and summaries per rate-limit row, inserting reset timestamps either inline or on continuation lines depending on width. Handles missing data (e.g., prompt to send a message).
  - `collect_rate_limit_labels` gathers labels so `FieldFormatter` can align columns consistently across rows.
- `HistoryCell::display_lines` constructs the final lines:
  - Header line shows “OpenAI Codex (vX.Y.Z)” with dimmed prompt marker.
  - Calculates `available_inner_width` (overall width minus border padding). Builds label set (Model, Directory, Approval, Sandbox, Agents.md, optional Account/Session/Token usage) and instantiates `FieldFormatter`.
  - Formats model details (including additional descriptors), directories via `format_directory_display`, approvals, sandbox, agents summary, account info (ChatGPT vs API key), session id, and token usage (hidden for ChatGPT accounts as they share platform billing).
  - Adds context window and rate limits, truncating lines via `truncate_line_to_width` to fit the inner width before passing through `with_border_with_inner_width` to draw the history cell border.

## Broader Context
- Invoked when users run `/status` in the TUI, providing a snapshot of workspace configuration, trust settings, and usage.
- Depends on shared helpers (`format.rs`, `helpers.rs`, `rate_limits.rs`) to keep styling consistent across status-related components.

## Technical Debt
- Rate-limit display assumes a relatively small set of rows; very large datasets could overflow the card without pagination.
- Token usage hiding for ChatGPT accounts is hard-coded; future plan types may necessitate more nuanced gating.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Introduce adaptive layout or pagination if rate-limit data grows beyond the card width.
related_specs:
  - helpers.rs.spec.md
  - format.rs.spec.md
  - rate_limits.rs.spec.md
