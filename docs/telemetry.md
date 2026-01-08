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

Each metric includes the required fields plus the global context above. Every metrics are prefixed by `codex.`.

| Metric            | Type    | Fields         | Description                                                              |
| ----------------- | ------- | -------------- | ------------------------------------------------------------------------ |
| `features.state`  | counter | `key`, `value` | Feature values that differ from defaults (emit one row per non-default). |
| `session.started` | counter | `is_git`       | New session created.                                                     |
| `task.compact`    | counter | `type`         | Number of compaction per type (`remote` or `local`)                      |
| `task.user_shell` | counter |                | Number of user shell actions (`!` in the TUI for example)                |
| `task.review`     | counter |                | Number of reviews triggered                                              |
| `task.undo`       | counter |                | Number of undo made                                                      |

### Metrics to be added

| Metric                    | Type      | Fields                                | Description                                               |
| ------------------------- | --------- | ------------------------------------- | --------------------------------------------------------- |
| `approval.requested`      | counter   | `tool`, `approved`                    | Tool approval request result (`approved`: `yes` or `no`). |
| `conversation.turn.count` | counter   |                                       | User/assistant turns per session.                         |
| `mcp.call`                | counter   | `status`                              | MCP tool invocation result (`ok` or error string).        |
| `model.call.duration_ms`  | histogram | `provider`, `status`, `attempt`       | Model API request duration.                               |
| `tool.call`               | counter   | `tool`, `status`                      | Tool invocation result (`ok` or error string).            |
| `tool.call.duration_ms`   | histogram | `tool`, `status`                      | Tool execution time.                                      |
| `user.feedback.submitted` | counter   | `category`, `include_logs`, `success` | Feedback submission via `/feedback`.                      |
