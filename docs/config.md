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

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Custom provider models

When you use a custom `model_provider`, you can now define a picker-visible model list with
`models = [...]` under that provider entry.

```toml
[model_providers.azure_foundry]
name = "Azure Foundry"
base_url = "https://YOUR_RESPONSES_GATEWAY/v1"
env_key = "FOUNDATION_MODEL_API_KEY"
wire_api = "responses"
models = ["kimi-k2", "deepseek-v3.2"]
```

For Azure AI Foundry chat-completions endpoints:

```toml
[model_providers.azure_foundry_chat]
name = "Azure Foundry Chat"
base_url = "https://YOUR_RESOURCE.services.ai.azure.com/models"
wire_api = "chat"
query_params = { api-version = "2024-05-01-preview" }
env_http_headers = { api-key = "AZURE_FOUNDRY_API_KEY" }
models = ["kimi-k2.5", "deepseek-v3.2", "gpt-5.2-chat"]
```

This is useful for providers that do not expose Codex-native model metadata.

Codex supports two wire protocols:

- `wire_api = "responses"` for providers exposing `/v1/responses`
- `wire_api = "chat"` for providers exposing `/v1/chat/completions`

You can also set up profile-based provider switching so startup command chooses
OpenAI subscription vs Azure Foundry:

```toml
model_provider = "openai"

[profiles.openai-subscription]
model_provider = "openai"

[profiles.azure-foundry]
model_provider = "azure_foundry_chat"
model = "deepseek-v3.2"
```

Run with a specific profile:

```bash
codex --profile openai-subscription
codex --profile azure-foundry
```

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
