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

## `model_info_overrides` precedence

`model_info_overrides` lets you patch metadata for specific model slugs in `config.toml`.
The patch is merge-only: only fields you set are overridden, and omitted fields keep their
resolved value.

Precedence for model metadata is:

1. Resolved model metadata (remote `/models` when available, otherwise built-in fallback)
2. `model_info_overrides[<slug>]`
3. Existing top-level config overrides (for example `model_context_window`,
   `model_auto_compact_token_limit`, `model_supports_reasoning_summaries`, and
   `tool_output_token_limit`)

This means top-level overrides still win for their specific fields after per-model patching.

Example:

```toml
[model_info_overrides.gpt-fake]
display_name = "gpt-fake-dev"
context_window = 400000
supports_parallel_tool_calls = true
base_instructions = "Custom model instructions"
```

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
