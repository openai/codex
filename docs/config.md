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

## Variable expansion

Codex expands environment variables in all `config.toml` string values and table keys.

- `$VAR` and `${VAR}` expand to environment variables.
- `$$` escapes to a literal `$`.
- `~` expands only when the string starts with `~/` or `~\\`.

If a variable is unset (including `HOME`/`USERPROFILE` for `~`), the value is left unchanged
and Codex emits a warning after loading the config.

When Codex persists project trust entries, it prefers updating an existing symbolic project key
(for example, `~` or `$HOME`) if it expands to the same directory, instead of adding a duplicate
absolute path entry.

When Codex checks whether a project is trusted, it also expands symbolic project keys (including
the bare `~` and `$VAR` forms) before matching against the current working directory. If both a
symbolic key and its absolute expansion exist for the same directory, Codex prefers the symbolic
entry and may remove the absolute entry when it only contains `trust_level`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
