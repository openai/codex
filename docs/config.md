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

## Tools

You can exclude specific tools from the model tool list via `[tools].disallowed_tools`. MCP tool
descriptions are deferred by default and exposed via `MCPSearch`; disable it if you want full MCP
descriptions included in the tool list. `MCPSearch` can also return tool schemas (when requested),
route a query to the best matching MCP tool (falling back to the latest user message when `query`
is omitted), and execute MCP tools via a `call` payload, which keeps MCP tools out of the tool list
while still allowing discovery and invocation. When
`MCPSearch` is enabled, direct MCP tool calls (such as
`@mcp__server__tool`) are routed through `MCPSearch` automatically. Use `route: true` when you
want heuristic selection; use `call` for explicit invocation:

```json
{"query":"exa","include_schema":true}
{"query":"search docs","route":true}
{"route":true}
{"call":{"qualified_name":"mcp__exa__web_search_exa","arguments":{"query":"foo"}}}
{"resources":{"action":"list","server":"figma"}}
{"resources":{"action":"read","server":"figma","uri":"memo://id"}}
```

```toml
[tools]
disallowed_tools = ["MCPSearch"]
```

## Agents

Configure subagent spawning behavior:

```toml
[agents]
max_threads = 4
subagent_model = "gpt-5.2-codex"
subagent_reasoning_effort = "high"
```

When `subagent_model` is unset, spawned subagents inherit the current session model.
When `subagent_reasoning_effort` is unset, spawned subagents inherit the current session reasoning effort.

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
