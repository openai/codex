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

## LSP

Codex can integrate with local Language Server Protocol (LSP) servers for diagnostics and semantic actions.
Configure this under `[lsp]` in `config.toml`:

```toml
[lsp]
mode = "auto" # off | auto | on
auto_install = false
install_dir = "$CODEX_HOME/lsp"
diagnostics_in_prompt = "errors" # off | errors | errors_and_warnings
max_diagnostics_per_file = 10
max_files = 5

[lsp.servers.rust-analyzer]
enabled = true
# command = "rust-analyzer"
```

CLI overrides:

- `--lsp=off|auto|on` to control LSP mode for a session
- `codex lsp status`, `codex lsp diagnostics`, `codex lsp install` for inspection and setup

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
