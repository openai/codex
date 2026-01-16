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

## Skills sources

You can add extra skill sources in `config.toml` using the `[skills]` table:

```toml
[skills]
sources = [
  { path = "/path/to/skills" },
  { url = "https://example.com/my-skill.skill" }
]
```

- `path` should point to a directory that contains one or more skill folders with `SKILL.md`.
- `url` can point to a `.skill` (zip) archive or a raw `SKILL.md`. Remote skills are cached under `~/.codex/skills/.remote/`.

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
