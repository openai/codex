# Configuration overview

Codex reads TOML config from ~/.codex/config.toml; see ../codex-rs/config.md for
 the full reference.
CLI flags override config (including --config key=value and --profile <name>); o
therwise values fall back to the file and then built‑in defaults.

## Using Open Source Models

Codex can run fully locally against an OpenAI-compatible OSS host (like Ollama) using the `--oss` flag:

- Interactive UI:
  - codex --oss
- Non-interactive (programmatic) mode:
  - echo "Refactor utils" | codex exec --oss

Model selection when using `--oss`:

- If you omit `-m/--model`, Codex defaults to -m gpt-oss:20b and will verify it exists locally (downloading if needed).
- To pick a different size, pass one of:
  - -m "gpt-oss:20b"
  - -m "gpt-oss:120b"

Point Codex at your own OSS host:

- By default, `--oss` talks to http://localhost:11434/v1.
- To use a different host, set one of these environment variables before running Codex:
  - CODEX_OSS_BASE_URL, for example:
    - CODEX_OSS_BASE_URL="http://my-ollama.example.com:11434/v1" codex --oss -m gpt-oss:20b
  - or CODEX_OSS_PORT (when the host is localhost):
    - CODEX_OSS_PORT=11434 codex --oss

Advanced: you can make this persist in your config instead of environment variables by overriding the built-in `oss` provider in `~/.codex/config.toml`:

```toml
[model_providers.oss]
name = "Open Source"
base_url = "http://my-ollama.example.com:11434/v1"
```

<details>
<summary><strong>Use <code>--profile</code> to configure model providers</strong></summary>

Codex also allows you to use other providers that support the OpenAI Chat Completions (or Responses) API.

To do so, you must first define custom [providers](../codex-rs/config.md#model_providers) in `~/.codex/config.toml`. For example, the provider for a standard Ollama setup would be defined as follows:

```toml
[model_providers.ollama]
name = "Ollama"
base_url = "http://localhost:11434/v1"
```

The `base_url` will have `/chat/completions` appended to it to build the full URL for the request.

For providers that also require an `Authorization` header of the form `Bearer: SECRET`, an `env_key` can be specified, which indicates the environment variable to read to use as the value of `SECRET` when making a request:

```toml
[model_providers.openrouter]
name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"
env_key = "OPENROUTER_API_KEY"
```

Providers that speak the Responses API are also supported by adding `wire_api = "responses"` as part of the definition. Accessing OpenAI models via Azure is an example of such a provider, though it also requires specifying additional `query_params` that need to be appended to the request URL:

```toml
[model_providers.azure]
name = "Azure"
# Make sure you set the appropriate subdomain for this URL.
base_url = "https://YOUR_PROJECT_NAME.openai.azure.com/openai"
env_key = "AZURE_OPENAI_API_KEY"  # Or "OPENAI_API_KEY", whichever you use.
# Newer versions appear to support the responses API, see https://github.com/openai/codex/pull/1321
query_params = { api-version = "2025-04-01-preview" }
wire_api = "responses"
```

Once you have defined a provider you wish to use, you can configure it as your default provider as follows:

```toml
model_provider = "azure"
```

> [!TIP]
> If you find yourself experimenting with a variety of models and providers, then you likely want to invest in defining a _profile_ for each configuration like so:

```toml
[profiles.o3]
model_provider = "azure"
model = "o3"

[profiles.mistral]
model_provider = "ollama"
model = "mistral"
```

This way, you can specify one command-line argument (.e.g., `--profile o3`, `--profile mistral`) to override multiple settings together.

</details>
<br />

## Security model & permissions

Codex lets you decide _how much autonomy_ you want to grant the agent. The following options can be configured independently:

- [`approval_policy`](../codex-rs/config.md#approval_policy) determines when you should be prompted to approve whether Codex can execute a command
- [`sandbox`](../codex-rs/config.md#sandbox) determines the _sandbox policy_ that Codex uses to execute untrusted commands

By default, Codex runs with `--ask-for-approval untrusted` and `--sandbox read-only`, which means that:

- The user is prompted to approve every command not on the set of "trusted" commands built into Codex (`cat`, `ls`, etc.)
- Approved commands are run outside of a sandbox because user approval implies "trust," in this case.

Running Codex with the `--full-auto` convenience flag changes the configuration to `--ask-for-approval on-failure` and `--sandbox workspace-write`, which means that:

- Codex does not initially ask for user approval before running an individual command.
- Though when it runs a command, it is run under a sandbox in which:
  - It can read any file on the system.
  - It can only write files under the current directory (or the directory specified via `--cd`).
  - Network requests are completely disabled.
- Only if the command exits with a non-zero exit code will it ask the user for approval. If granted, it will re-attempt the command outside of the sandbox. (A common case is when Codex cannot `npm install` a dependency because that requires network access.)

Again, these two options can be configured independently. For example, if you want Codex to perform an "exploration" where you are happy for it to read anything it wants but you never want to be prompted, you could run Codex with `--ask-for-approval never` and `--sandbox read-only`.

### Platform sandboxing details

The mechanism Codex uses to implement the sandbox policy depends on your OS:

- **macOS 12+** uses **Apple Seatbelt** and runs commands using `sandbox-exec` with a profile (`-p`) that corresponds to the `--sandbox` that was specified.
- **Linux** uses a combination of Landlock/seccomp APIs to enforce the `sandbox` configuration.

Note that when running Linux in a containerized environment such as Docker, sandboxing may not work if the host/container configuration does not support the necessary Landlock/seccomp APIs. In such cases, we recommend configuring your Docker container so that it provides the sandbox guarantees you are looking for and then running `codex` with `--sandbox danger-full-access` (or, more simply, the `--dangerously-bypass-approvals-and-sandbox` flag) within your container.

---

## Model Context Protocol (MCP)

The Codex CLI can be configured to leverage MCP servers by defining an [`mcp_servers`](../codex-rs/config.md#mcp_servers) section in `~/.codex/config.toml`. It is intended to mirror how tools such as Claude and Cursor define `mcpServers` in their respective JSON config files, though the Codex format is slightly different since it uses TOML rather than JSON, e.g.:

```toml
# IMPORTANT: the top-level key is `mcp_servers` rather than `mcpServers`.
[mcp_servers.server-name]
command = "npx"
args = ["-y", "mcp-server"]
env = { "API_KEY" = "value" }
```

> [!TIP]
> It is somewhat experimental, but the Codex CLI can also be run as an MCP _server_ via `codex mcp`. If you launch it with an MCP client such as `npx @modelcontextprotocol/inspector codex mcp` and send it a `tools/list` request, you will see that there is only one tool, `codex`, that accepts a grab-bag of inputs, including a catch-all `config` map for anything you might want to override. Feel free to play around with it and provide feedback via GitHub issues.

---

## Zero data retention (ZDR) usage

Codex CLI **does** support OpenAI organizations with [Zero Data Retention (ZDR)](https://platform.openai.com/docs/guides/your-data#zero-data-retention) enabled. If your OpenAI organization has Zero Data Retention enabled and you still encounter errors such as:

```
OpenAI rejected the request. Error details: Status: 400, Code: unsupported_parameter, Type: invalid_request_error, Message: 400 Previous response cannot be used for this organization due to Zero Data Retention.
```

Ensure you are running `codex` with `--config disable_response_storage=true` or add this line to `~/.codex/config.toml` to avoid specifying the command line option each time:

```toml
disable_response_storage = true
```

See [the configuration documentation on `disable_response_storage`](../codex-rs/config.md#disable_response_storage) for details.

---