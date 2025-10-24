## Overview
`codex-tui::session_log` records high-fidelity session logs when `CODEX_TUI_RECORD_SESSION` is enabled. It writes JSONL records capturing app events and outbound ops so sessions can be replayed or analyzed later.

## Detailed Behavior
- `SessionLogger` (lazy singleton):
  - Holds a lazily initialized `File` guarded by `Mutex`.
  - `open(path)` creates/truncates the log file (chmod 600 on Unix) and stores the file handle.
  - `write_json_line(value)` serializes JSON, writes a line, and flushes; logs warnings on errors.
  - `is_enabled` checks whether logging is active.
- Initialization:
  - `maybe_init(config)` checks `CODEX_TUI_RECORD_SESSION`. If true, determines the log path (env override or `log_dir`), opens the file, and writes a session metadata header (timestamp, cwd, model, provider).
- Logging functions:
  - `log_inbound_app_event(event)`: writes summaries when events arrive from the app layer (including Codex events, file search actions, history cell insertions). Non-core events are logged with variant names.
  - `log_outbound_op(op)`: records user operations sent to Codex.
  - `log_session_end()`: appends a closing record when the session ends.
- Utilities:
  - `now_ts()` returns RFC3339 timestamps with millisecond precision.
  - `write_record` serializes objects using `serde_json::to_value` and writes them via the logger.

## Broader Context
- `AppEventSender::send` invokes `log_inbound_app_event` for most events. The chat widget logs outbound ops when submitting requests. `run_ratatui_app` calls `log_session_end` during teardown.
- Log files aid debugging and telemetry; they can be analyzed to reconstruct user flows or reproduce issues.
- Context can't yet be determined for structured replay tools; current output is geared toward offline analysis.

## Technical Debt
- Logs capture limited metadata for some events (variant name only); consider enriching records with structured payloads as replay tooling matures.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add structured payloads for currently summarized events to support richer replay and debugging.
related_specs:
  - ./app_event.rs.spec.md
  - ./app_event_sender.rs.spec.md
