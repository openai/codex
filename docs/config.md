# Configuration

For basic configuration instructions, see [this documentation](https://developers.openai.com/codex/config-basic).

For advanced configuration instructions, see [this documentation](https://developers.openai.com/codex/config-advanced).

For a full configuration reference, see [this documentation](https://developers.openai.com/codex/config-reference).

## Connecting to MCP servers

Codex can connect to MCP servers configured in `~/.codex/config.toml`. See the configuration reference for the latest MCP server options:

- https://developers.openai.com/codex/config-reference

## Notify

Codex can run a notification hook when the agent finishes a turn. See the configuration reference for the latest notification settings:

- https://developers.openai.com/codex/config-reference

## Web search (Tavily)

Enable Tavily-backed web search by setting the feature flag and adding your API key:

```toml
[features]
web_search_request = true

tavily_api_key = "tvly-..."
```

When enabled, the model can call the `web_search` tool. It defaults to 10 results unless the tool call overrides `limit`.

For the full configuration reference, see:

- https://developers.openai.com/codex/config-reference
