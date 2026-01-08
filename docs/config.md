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

## Tool environment variables

When Codex runs shell-based tools (for example `shell`, `local_shell`, or `exec_command`), it injects:

- `CODEX_CONVERSATION_ID` (the session/window id)
- `CODEX_TURN_ID` (the current turn id)
- `CODEX_CWD` (the resolved working directory for the command)
