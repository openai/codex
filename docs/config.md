# Configuration

For basic configuration instructions, see [this documentation](https://developers.openai.com/codex/config-basic).

For advanced configuration instructions, see [this documentation](https://developers.openai.com/codex/config-advanced).

For a full configuration reference, see [this documentation](https://developers.openai.com/codex/config-reference).

## Connecting to MCP servers

Codex can connect to MCP servers configured in `~/.codex/config.toml`. See the configuration reference for the latest MCP server options:

- https://developers.openai.com/codex/config-reference

## Apps (Connectors)

Use `$` in the composer to insert a ChatGPT connector; the popover lists accessible
apps. The `/apps` command lists available and installed apps. Connected apps appear first
and are labeled as connected; others are marked as can be installed.

## Notify

Codex can run a notification hook when the agent finishes a turn. See the configuration reference for the latest notification settings:

- https://developers.openai.com/codex/config-reference

When Codex knows which client started the turn, the legacy notify JSON payload also includes a top-level `client` field. The TUI reports `codex-tui`, and the app server reports the `clientInfo.name` value from `initialize`.

## Lifecycle Hooks

Codex can run command hooks for lifecycle events via `[hooks]` in `~/.codex/config.toml`:

```toml
[hooks]
session_start = ["my-hook", "session_start"]
user_prompt_submit = ["my-hook", "user_prompt_submit"]
pre_tool_use = ["my-hook", "pre_tool_use"]
post_tool_use = ["my-hook", "post_tool_use"]
stop = ["my-hook", "stop"]
pre_compact = ["my-hook", "pre_compact"]
session_end = ["my-hook", "session_end"]
subagent_start = ["my-hook", "subagent_start"]
subagent_stop = ["my-hook", "subagent_stop"]
```

Each command receives JSON on stdin describing the hook payload.

To align with the Claude-compatible event naming, the hook names are:

- `turn_start` becomes `user_prompt_submit`
- `turn_end` becomes `stop`
- `compaction` becomes `pre_compact`
- `subagent_end` becomes `subagent_stop`

These older names never landed on `main`, so they are not accepted as aliases.

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## SQLite State DB

Codex stores the SQLite-backed state DB under `sqlite_home` (config key) or the
`CODEX_SQLITE_HOME` environment variable. When unset, WorkspaceWrite sandbox
sessions default to a temp directory; other modes default to `CODEX_HOME`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

## Plan mode defaults

`plan_mode_reasoning_effort` lets you set a Plan-mode-specific default reasoning
effort override. When unset, Plan mode uses the built-in Plan preset default
(currently `medium`). When explicitly set (including `none`), it overrides the
Plan preset. The string value `none` means "no reasoning" (an explicit Plan
override), not "inherit the global default". There is currently no separate
config value for "follow the global default in Plan mode".

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
