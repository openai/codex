# Configuration

For basic configuration instructions, see [this documentation](https://developers.openai.com/codex/config-basic).

For advanced configuration instructions, see [this documentation](https://developers.openai.com/codex/config-advanced).

For a full configuration reference, see [this documentation](https://developers.openai.com/codex/config-reference).

## Connecting to MCP servers

Codex can connect to MCP servers configured in `~/.codex/config.toml`. See the configuration reference for the latest MCP server options:

- https://developers.openai.com/codex/config-reference

### Passing environment variables to MCP servers

MCP server arguments (`args`) are passed directly to the process — they are **not** interpreted by a shell, so `$VAR` or `${VAR}` in an `args` value will be passed literally, not expanded.

To securely supply API keys or secrets to an MCP server, use one of two approaches:

**Option 1 – static value in `[mcp_servers.NAME.env]`:**

```toml
[mcp_servers.my-server]
command = "npx"
args = ["-y", "my-mcp-package"]

[mcp_servers.my-server.env]
MY_API_KEY = "sk-..."
```

**Option 2 – inherit a variable from the host environment via `env_vars`:**

```toml
[mcp_servers.my-server]
command = "npx"
args    = ["-y", "my-mcp-package"]
env_vars = ["MY_API_KEY"]
```

With `env_vars`, Codex reads `MY_API_KEY` from its own environment and passes it to the MCP server process. This avoids storing secrets in `config.toml`.

> **Note:** If Codex is not started from a shell that already exports the variable (e.g. it is launched from a GUI), the variable may not be present. In that case, consider setting it in `~/.codex/config.toml` under `[env]`, or exporting it in the shell that launches Codex.

> **Note on `shell_environment_policy`:** By default, Codex restricts which variables are forwarded to subprocesses. If a variable exported by your shell is not reaching the MCP server, check your `shell_environment_policy` settings — you may need to add the variable name to the allowlist or set `ignore_default_excludes = true`. See [#3064](https://github.com/openai/codex/issues/3064) for context.

## Apps (Connectors)

Use `$` in the composer to insert a ChatGPT connector; the popover lists accessible
apps. The `/apps` command lists available and installed apps. Connected apps appear first
and are labeled as connected; others are marked as can be installed.

## Notify

Codex can run a notification hook when the agent finishes a turn. See the configuration reference for the latest notification settings:

- https://developers.openai.com/codex/config-reference

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
