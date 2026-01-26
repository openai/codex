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

## Collaboration mode presets

You can add custom collaboration modes that are selectable via Shift+Tab in the TUI:

```toml
collaboration_mode_palette = ["magenta", "cyan", "green", "red"]

[collaboration_mode_presets."Research"]
developer_instructions = "Focus on evidence and cite sources."
reasoning_effort = "high"
color = "magenta"

[collaboration_mode_presets."Spec"]
developer_instructions_file = "/path/to/spec.md"
```

Each preset must define exactly one of `developer_instructions` or `developer_instructions_file`.
Colors are optional; supported values are `cyan`, `magenta`, `green`, and `red`. When a preset
does not specify a color, Codex assigns a deterministic color based on the preset name and the
`collaboration_mode_palette`. The palette must be non-empty; if omitted, Codex uses the default
order shown above so the same mode keeps its color across sessions.
Custom names cannot conflict with built-in modes (Plan, Code, Pair Programming, Execute). Built-ins
appear first; custom presets follow in name order.

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
