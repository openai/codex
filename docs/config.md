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

## Collaboration modes

When the `collaboration_modes` feature is enabled, Codex uses built-in presets for `plan`, `pair_programming`, and `execute`.

You can override the model and reasoning effort globally via `model` / `model_reasoning_effort`, and optionally per-mode:

```toml
model = "gpt-5.2"
model_reasoning_effort = "xhigh"

[collaboration_modes.plan]
model = "gpt-5.2"
model_reasoning_effort = "high"

[collaboration_modes.execute]
model_reasoning_effort = "xhigh"
```

Precedence is: built-in preset < global overrides < per-mode overrides.

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
