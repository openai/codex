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

## Exec policy auto-allow prefixes

You can pre-approve exec command prefixes to avoid repeated approvals. Prefixes are tokenized with shell rules and applied to exec approvals only. See the configuration reference for the latest options:

Example:

```toml
[execpolicy]
auto_allow_prefixes = [
  "PGPASSWORD=example_password psql -h 127.0.0.1 -p 5432 -U example_user -d example_db -c",
  "git status",
]
```

Note: this affects exec approvals only; other tools (like apply_patch) are unchanged.
