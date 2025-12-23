# Codex Telemetry

## Config

**TODO(jif)**: add the config and document it

## Tracing

Codex can export OpenTelemetry **log events**, **trace spans**, and **metrics**
when OTEL exporters are configured in `config.toml` (`[otel]`).
By default, exporters are disabled and nothing is sent.

## Feedback

Feedback is sent only when you run `/feedback` and confirm. The report includes
the selected category and optional note; if you opt in to include logs, Codex
attaches the most recent in-memory logs for the session (up to ~4 MiB).

## Metrics

This section list all the metrics exported by Codex when locally installed.

### Global context (applies to every event/metric)

- `surface`: `cli` | `vscode` | `exec` | `mcp` | `subagent_*` (from `SessionSource`).
- `version`: binary version.
- `auth_mode`: `swic` (AuthMode::ChatGPT) | `api` (AuthMode::ApiKey) | `unknown`.
- `model`: name of the model used.

## Metrics catalog

Each metric includes the required fields plus the global context above.

| Metric                    | Type      | Fields                                | Description                                                                     |
| ------------------------- | --------- | ------------------------------------- | ------------------------------------------------------------------------------- |
| `approval.requested`      | counter   | `tool`, `approved`                    | Tool approval request result (`approved`: `yes` or `no`).                       |
| `auth.completed`          | counter   | `status`                              | Authentication completed (only for ChatGPT authentication).                     |
| `conversation.compact`    | counter   | `status`, `number`                    | Compaction event including the status and the compaction number in the session. |
| `conversation.turn.count` | counter   | `role`                                | User/assistant turns per session.                                               |
| `feature.duration_ms`     | histogram | `feature`, `status`                   | End-to-end feature latency.                                                     |
| `feature.used`            | counter   | `feature`                             | Feature usage through `/` (e.g., `/undo`, `/review`, ...).                      |
| `features.state`          | counter   | `key`, `value`                        | Feature values that differ from defaults (emit one row per non-default).        |
| `mcp.call`                | counter   | `status`                              | MCP tool invocation result (`ok` or error string).                              |
| `model.call.duration_ms`  | histogram | `provider`, `status`, `attempt`       | Model API request duration.                                                     |
| `session.started`         | counter   | `is_git`                              | New session created.                                                            |
| `tool.call`               | counter   | `tool`, `status`                      | Tool invocation result (`ok` or error string).                                  |
| `tool.call.duration_ms`   | histogram | `tool`, `status`                      | Tool execution time.                                                            |
| `user.feedback.submitted` | counter   | `category`, `include_logs`, `success` | Feedback submission via `/feedback`.                                            |
