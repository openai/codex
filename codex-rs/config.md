# Config

Codex supports several mechanisms for setting config values:

- Config-specific command-line flags, such as `--model o3` (highest precedence).
- A generic `-c`/`--config` flag that takes a `key=value` pair, such as `--config model="o3"`.
  - The key can contain dots to set a value deeper than the root, e.g. `--config model_providers.openai.wire_api="chat"`.
  - Values can contain objects, such as `--config shell_environment_policy.include_only=["PATH", "HOME", "USER"]`.
  - For consistency with `config.toml`, values are in TOML format rather than JSON format, so use `{a = 1, b = 2}` rather than `{"a": 1, "b": 2}`.
  - If `value` cannot be parsed as a valid TOML value, it is treated as a string value. This means that both `-c model="o3"` and `-c model=o3` are equivalent.
- The `$CODEX_HOME/config.toml` configuration file where the `CODEX_HOME` environment value defaults to `~/.codex`. (Note `CODEX_HOME` will also be where logs and other Codex-related information are stored.)

Both the `--config` flag and the `config.toml` file support the following options:

## model

The model that Codex should use.

```toml
model = "o3"  # overrides the default of "codex-mini-latest"
```

## model_providers

This option lets you override and amend the default set of model providers bundled with Codex. This value is a map where the key is the value to use with `model_provider` to select the corresponding provider.

For example, if you wanted to add a provider that uses the OpenAI 4o model via the chat completions API, then you could add the following configuration:

```toml
# Recall that in TOML, root keys must be listed before tables.
model = "gpt-4o"
model_provider = "openai-chat-completions"

[model_providers.openai-chat-completions]
# Name of the provider that will be displayed in the Codex UI.
name = "OpenAI using Chat Completions"
# The path `/chat/completions` will be amended to this URL to make the POST
# request for the chat completions.
base_url = "https://api.openai.com/v1"
# If `env_key` is set, identifies an environment variable that must be set when
# using Codex with this provider. The value of the environment variable must be
# non-empty and will be used in the `Bearer TOKEN` HTTP header for the POST request.
env_key = "OPENAI_API_KEY"
# Valid values for wire_api are "chat" and "responses". Defaults to "chat" if omitted.
wire_api = "chat"
# If necessary, extra query params that need to be added to the URL.
# See the Azure example below.
query_params = {}
```

Note this makes it possible to use Codex CLI with non-OpenAI models, so long as they use a wire API that is compatible with the OpenAI chat completions API. For example, you could define the following provider to use Codex CLI with Ollama running locally:

```toml
[model_providers.ollama]
name = "Ollama"
base_url = "http://localhost:11434/v1"
```

Or a third-party provider (using a distinct environment variable for the API key):

```toml
[model_providers.mistral]
name = "Mistral"
base_url = "https://api.mistral.ai/v1"
env_key = "MISTRAL_API_KEY"
```

Note that Azure requires `api-version` to be passed as a query parameter, so be sure to specify it as part of `query_params` when defining the Azure provider:

```toml
[model_providers.azure]
name = "Azure"
# Make sure you set the appropriate subdomain for this URL.
base_url = "https://YOUR_PROJECT_NAME.openai.azure.com/openai"
env_key = "AZURE_OPENAI_API_KEY"  # Or "OPENAI_API_KEY", whichever you use.
query_params = { api-version = "2025-04-01-preview" }
```

It is also possible to configure a provider to include extra HTTP headers with a request. These can be hardcoded values (`http_headers`) or values read from environment variables (`env_http_headers`):

```toml
[model_providers.example]
# name, base_url, ...

# This will add the HTTP header `X-Example-Header` with value `example-value`
# to each request to the model provider.
http_headers = { "X-Example-Header" = "example-value" }

# This will add the HTTP header `X-Example-Features` with the value of the
# `EXAMPLE_FEATURES` environment variable to each request to the model provider
# _if_ the environment variable is set and its value is non-empty.
env_http_headers = { "X-Example-Features": "EXAMPLE_FEATURES" }
```

### Per-provider network tuning

The following optional settings control retry behaviour and streaming idle timeouts **per model provider**. They must be specified inside the corresponding `[model_providers.<id>]` block in `config.toml`. (Older releases accepted top‑level keys; those are now ignored.)

Example:

```toml
[model_providers.openai]
name = "OpenAI"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
# network tuning overrides (all optional; falls back to built‑in defaults)
request_max_retries = 4            # retry failed HTTP requests
stream_max_retries = 10            # retry dropped SSE streams
stream_idle_timeout_ms = 300000    # 5m idle timeout
```

#### request_max_retries
How many times Codex will retry a failed HTTP request to the model provider. Defaults to `4`.

#### stream_max_retries
Number of times Codex will attempt to reconnect when a streaming response is interrupted. Defaults to `10`.

#### stream_idle_timeout_ms
How long Codex will wait for activity on a streaming response before treating the connection as lost. Defaults to `300_000` (5 minutes).

## model_provider

Identifies which provider to use from the `model_providers` map. Defaults to `"openai"`. You can override the `base_url` for the built-in `openai` provider via the `OPENAI_BASE_URL` environment variable.

Note that if you override `model_provider`, then you likely want to override
`model`, as well. For example, if you are running ollama with Mistral locally,
then you would need to add the following to your config in addition to the new entry in the `model_providers` map:

```toml
model_provider = "ollama"
model = "mistral"
```

## approval_policy

Determines when the user should be prompted to approve whether Codex can execute a command:

```toml
# Codex has hardcoded logic that defines a set of "trusted" commands.
# Setting the approval_policy to `untrusted` means that Codex will prompt the
# user before running a command not in the "trusted" set.
#
# See https://github.com/openai/codex/issues/1260 for the plan to enable
# end-users to define their own trusted commands.
approval_policy = "untrusted"
```

```toml
# If the command fails when run in the sandbox, Codex asks for permission to
# retry the command outside the sandbox.
approval_policy = "on-failure"
```

```toml
# User is never prompted: if the command fails, Codex will automatically try
# something out. Note the `exec` subcommand always uses this mode.
approval_policy = "never"
```

## profiles

A _profile_ is a collection of configuration values that can be set together. Multiple profiles can be defined in `config.toml` and you can specify the one you
want to use at runtime via the `--profile` flag.

Here is an example of a `config.toml` that defines multiple profiles:

```toml
model = "o3"
approval_policy = "unless-allow-listed"
disable_response_storage = false

# Setting `profile` is equivalent to specifying `--profile o3` on the command
# line, though the `--profile` flag can still be used to override this value.
profile = "o3"

[model_providers.openai-chat-completions]
name = "OpenAI using Chat Completions"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
wire_api = "chat"

[profiles.o3]
model = "o3"
model_provider = "openai"
approval_policy = "never"
model_reasoning_effort = "high"
model_reasoning_summary = "detailed"

[profiles.gpt3]
model = "gpt-3.5-turbo"
model_provider = "openai-chat-completions"

[profiles.zdr]
model = "o3"
model_provider = "openai"
approval_policy = "on-failure"
disable_response_storage = true
```

Users can specify config values at multiple levels. Order of precedence is as follows:

1. custom command-line argument, e.g., `--model o3`
2. as part of a profile, where the `--profile` is specified via a CLI (or in the config file itself)
3. as an entry in `config.toml`, e.g., `model = "o3"`
4. the default value that comes with Codex CLI (i.e., Codex CLI defaults to `codex-mini-latest`)

## model_reasoning_effort

If the model name starts with `"o"` (as in `"o3"` or `"o4-mini"`) or `"codex"`, reasoning is enabled by default when using the Responses API. As explained in the [OpenAI Platform documentation](https://platform.openai.com/docs/guides/reasoning?api-mode=responses#get-started-with-reasoning), this can be set to:

- `"low"`
- `"medium"` (default)
- `"high"`

To disable reasoning, set `model_reasoning_effort` to `"none"` in your config:

```toml
model_reasoning_effort = "none"  # disable reasoning
```

## model_reasoning_summary

If the model name starts with `"o"` (as in `"o3"` or `"o4-mini"`) or `"codex"`, reasoning is enabled by default when using the Responses API. As explained in the [OpenAI Platform documentation](https://platform.openai.com/docs/guides/reasoning?api-mode=responses#reasoning-summaries), this can be set to:

- `"auto"` (default)
- `"concise"`
- `"detailed"`

To disable reasoning summaries, set `model_reasoning_summary` to `"none"` in your config:

```toml
model_reasoning_summary = "none"  # disable reasoning summaries
```

## model_supports_reasoning_summaries

By default, `reasoning` is only set on requests to OpenAI models that are known to support them. To force `reasoning` to set on requests to the current model, you can force this behavior by setting the following in `config.toml`:

```toml
model_supports_reasoning_summaries = true
```

## sandbox_mode

Codex executes model-generated shell commands inside an OS-level sandbox.

In most cases you can pick the desired behaviour with a single option:

```toml
# same as `--sandbox read-only`
sandbox_mode = "read-only"
```

The default policy is `read-only`, which means commands can read any file on
disk, but attempts to write a file or access the network will be blocked.

A more relaxed policy is `workspace-write`. When specified, the current working directory for the Codex task will be writable (as well as `$TMPDIR` on macOS). Note that the CLI defaults to using the directory where it was spawned as `cwd`, though this can be overridden using `--cwd/-C`.

```toml
# same as `--sandbox workspace-write`
sandbox_mode = "workspace-write"

# Extra settings that only apply when `sandbox = "workspace-write"`.
[sandbox_workspace_write]
# By default, only the cwd for the Codex session will be writable (and $TMPDIR
# on macOS), but you can specify additional writable folders in this array.
writable_roots = ["/tmp"]
# Allow the command being run inside the sandbox to make outbound network
# requests. Disabled by default.
network_access = false
```

To disable sandboxing altogether, specify `danger-full-access` like so:

```toml
# same as `--sandbox danger-full-access`
sandbox_mode = "danger-full-access"
```

This is reasonable to use if Codex is running in an environment that provides its own sandboxing (such as a Docker container) such that further sandboxing is unnecessary.

Though using this option may also be necessary if you try to use Codex in environments where its native sandboxing mechanisms are unsupported, such as older Linux kernels or on Windows.

## mcp_servers

Defines the list of MCP servers that Codex can consult for tool use. Currently, only servers that are launched by executing a program that communicate over stdio are supported. For servers that use the SSE transport, consider an adapter like [mcp-proxy](https://github.com/sparfenyuk/mcp-proxy).

**Note:** Codex may cache the list of tools and resources from an MCP server so that Codex can include this information in context at startup without spawning all the servers. This is designed to save resources by loading MCP servers lazily.

This config option is comparable to how Claude and Cursor define `mcpServers` in their respective JSON config files, though because Codex uses TOML for its config language, the format is slightly different. For example, the following config in JSON:

```json
{
  "mcpServers": {
    "server-name": {
      "command": "npx",
      "args": ["-y", "mcp-server"],
      "env": {
        "API_KEY": "value"
      }
    }
  }
}
```

Should be represented as follows in `~/.codex/config.toml`:

```toml
# IMPORTANT: the top-level key is `mcp_servers` rather than `mcpServers`.
[mcp_servers.server-name]
command = "npx"
args = ["-y", "mcp-server"]
env = { "API_KEY" = "value" }
```

## disable_response_storage

Currently, customers whose accounts are set to use Zero Data Retention (ZDR) must set `disable_response_storage` to `true` so that Codex uses an alternative to the Responses API that works with ZDR:

```toml
disable_response_storage = true
```

## shell_environment_policy

Codex spawns subprocesses (e.g. when executing a `local_shell` tool-call suggested by the assistant). By default it passes **only a minimal core subset** of your environment to those subprocesses to avoid leaking credentials. You can tune this behavior via the **`shell_environment_policy`** block in
`config.toml`:

```toml
[shell_environment_policy]
# inherit can be "core" (default), "all", or "none"
inherit = "core"
# set to true to *skip* the filter for `"*KEY*"` and `"*TOKEN*"`
ignore_default_excludes = false
# exclude patterns (case-insensitive globs)
exclude = ["AWS_*", "AZURE_*"]
# force-set / override values
set = { CI = "1" }
# if provided, *only* vars matching these patterns are kept
include_only = ["PATH", "HOME"]
```

| Field                     | Type                       | Default | Description                                                                                                                                     |
| ------------------------- | -------------------------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `inherit`                 | string                     | `core`  | Starting template for the environment:<br>`core` (`HOME`, `PATH`, `USER`, …), `all` (clone full parent env), or `none` (start empty).           |
| `ignore_default_excludes` | boolean                    | `false` | When `false`, Codex removes any var whose **name** contains `KEY`, `SECRET`, or `TOKEN` (case-insensitive) before other rules run.              |
| `exclude`                 | array&lt;string&gt;        | `[]`    | Case-insensitive glob patterns to drop after the default filter.<br>Examples: `"AWS_*"`, `"AZURE_*"`.                                           |
| `set`                     | table&lt;string,string&gt; | `{}`    | Explicit key/value overrides or additions – always win over inherited values.                                                                   |
| `include_only`            | array&lt;string&gt;        | `[]`    | If non-empty, a whitelist of patterns; only variables that match _one_ pattern survive the final step. (Generally used with `inherit = "all"`.) |

The patterns are **glob style**, not full regular expressions: `*` matches any
number of characters, `?` matches exactly one, and character classes like
`[A-Z]`/`[^0-9]` are supported. Matching is always **case-insensitive**. This
syntax is documented in code as `EnvironmentVariablePattern` (see
`core/src/config_types.rs`).

If you just need a clean slate with a few custom entries you can write:

```toml
[shell_environment_policy]
inherit = "none"
set = { PATH = "/usr/bin", MY_FLAG = "1" }
```

Currently, `CODEX_SANDBOX_NETWORK_DISABLED=1` is also added to the environment, assuming network is disabled. This is not configurable.

## notify

Specify a program that will be executed to get notified about events generated by Codex. Note that the program will receive the notification argument as a string of JSON, e.g.:

```json
{
  "type": "agent-turn-complete",
  "turn-id": "12345",
  "input-messages": ["Rename `foo` to `bar` and update the callsites."],
  "last-assistant-message": "Rename complete and verified `cargo build` succeeds."
}
```

The `"type"` property will always be set. Currently, `"agent-turn-complete"` is the only notification type that is supported.

As an example, here is a Python script that parses the JSON and decides whether to show a desktop push notification using [terminal-notifier](https://github.com/julienXX/terminal-notifier) on macOS:

```python
#!/usr/bin/env python3

import json
import subprocess
import sys


def main() -> int:
    if len(sys.argv) != 2:
        print("Usage: notify.py <NOTIFICATION_JSON>")
        return 1

    try:
        notification = json.loads(sys.argv[1])
    except json.JSONDecodeError:
        return 1

    match notification_type := notification.get("type"):
        case "agent-turn-complete":
            assistant_message = notification.get("last-assistant-message")
            if assistant_message:
                title = f"Codex: {assistant_message}"
            else:
                title = "Codex: Turn Complete!"
            input_messages = notification.get("input_messages", [])
            message = " ".join(input_messages)
            title += message
        case _:
            print(f"not sending a push notification for: {notification_type}")
            return 0

    subprocess.check_output(
        [
            "terminal-notifier",
            "-title",
            title,
            "-message",
            message,
            "-group",
            "codex",
            "-ignoreDnD",
            "-activate",
            "com.googlecode.iterm2",
        ]
    )

    return 0


if __name__ == "__main__":
    sys.exit(main())
```

To have Codex use this script for notifications, you would configure it via `notify` in `~/.codex/config.toml` using the appropriate path to `notify.py` on your computer:

```toml
notify = ["python3", "/Users/mbolin/.codex/notify.py"]
```

## history

By default, Codex CLI records messages sent to the model in `$CODEX_HOME/history.jsonl`. Note that on UNIX, the file permissions are set to `o600`, so it should only be readable and writable by the owner.

To disable this behavior, configure `[history]` as follows:

```toml
[history]
persistence = "none"  # "save-all" is the default value
```

## file_opener

Identifies the editor/URI scheme to use for hyperlinking citations in model output. If set, citations to files in the model output will be hyperlinked using the specified URI scheme so they can be ctrl/cmd-clicked from the terminal to open them.

For example, if the model output includes a reference such as `【F:/home/user/project/main.py†L42-L50】`, then this would be rewritten to link to the URI `vscode://file/home/user/project/main.py:42`.

Note this is **not** a general editor setting (like `$EDITOR`), as it only accepts a fixed set of values:

- `"vscode"` (default)
- `"vscode-insiders"`
- `"windsurf"`
- `"cursor"`
- `"none"` to explicitly disable this feature

Currently, `"vscode"` is the default, though Codex does not verify VS Code is installed. As such, `file_opener` may default to `"none"` or something else in the future.

## hide_agent_reasoning

Codex intermittently emits "reasoning" events that show the model's internal "thinking" before it produces a final answer. Some users may find these events distracting, especially in CI logs or minimal terminal output.

Setting `hide_agent_reasoning` to `true` suppresses these events in **both** the TUI as well as the headless `exec` sub-command:

```toml
hide_agent_reasoning = true   # defaults to false
```

## model_context_window

The size of the context window for the model, in tokens.

In general, Codex knows the context window for the most common OpenAI models, but if you are using a new model with an old version of the Codex CLI, then you can use `model_context_window` to tell Codex what value to use to determine how much context is left during a conversation.

## model_max_output_tokens

This is analogous to `model_context_window`, but for the maximum number of output tokens for the model.

## project_doc_max_bytes

Maximum number of bytes to read from an `AGENTS.md` file to include in the instructions sent with the first turn of a session. Defaults to 32 KiB.

## tui

Options that are specific to the TUI.

```toml
[tui]
# More to come here
```
