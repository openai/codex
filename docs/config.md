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

`apps` supports layered defaults and per-tool settings:

```toml
[apps._default]
disable_destructive = false
disable_open_world = false

[apps.connector_123]
enabled = false
disabled_reason = "user"
disable_destructive = true
disable_open_world = true

[apps.connector_123.tools._default]
enabled = true
approval = "prompt" # "auto" | "prompt" | "approve"

[apps.connector_123.tools."repos/list"]
enabled = true
approval = "auto"
```

Tool approval modes:

- `auto`: Let the tool decide when to ask for approval.
- `prompt`: always ask for approval.
- `approve`: never ask for approval.

## Notify

Codex can run a notification hook when the agent finishes a turn. See the configuration reference for the latest notification settings:

- https://developers.openai.com/codex/config-reference

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
