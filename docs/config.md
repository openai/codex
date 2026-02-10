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

## Agent Inbox Delivery

Inbound messages from other agents (for example, when a watchdog uses `send_input`) can be injected
as different roles in the root thread. Configure this under the `[agents]` table:

```toml
[agents]
inbox_delivery_role = "tool" # tool | developer | assistant
```

`tool` (default) injects a synthetic `collab_inbox` tool-call + tool-output pair so inbound agent
messages are explicit tool activity in the transcript. `developer` injects a developer message with
an explicit `[collab_inbox:...]` prefix so it is not mistaken for user input. `assistant` injects
the same prefix using the assistant role.

This setting applies to non-subagent threads (for example, the root thread). Messages sent to
subagents via `send_input` are still delivered as user input.

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
