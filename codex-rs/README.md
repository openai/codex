# codex-rs

April 24, 2025

Today, Codex CLI is written in TypeScript and requires Node.js 22+ to run it. For a number of users, this runtime requirement inhibits adoption: they would be better served by a standalone executable. As maintainers, we want Codex to run efficiently in a wide range of environments with minimal overhead. We also want to take advantage of operating system-specific APIs to provide better sandboxing, where possible.

To that end, we are moving forward with a Rust implementation of Codex CLI contained in this folder, which has the following benefits:

- The CLI compiles to small, standalone, platform-specific binaries.
- Can make direct, native calls to [seccomp](https://man7.org/linux/man-pages/man2/seccomp.2.html) and [landlock](https://man7.org/linux/man-pages/man7/landlock.7.html) in order to support sandboxing on Linux.
- No runtime garbage collection, resulting in lower memory consumption and better, more predictable performance.

Currently, the Rust implementation is materially behind the TypeScript implementation in functionality, so continue to use the TypeScript implementation for the time being. We will publish native executables via GitHub Releases as soon as we feel the Rust version is usable.

## Code Organization

This folder is the root of a Cargo workspace. It contains quite a bit of experimental code, but here are the key crates:

- [`core/`](./core) contains the business logic for Codex. Ultimately, we hope this to be a library crate that is generally useful for building other Rust/native applications that use Codex.
- [`exec/`](./exec) "headless" CLI for use in automation.
- [`tui/`](./tui) CLI that launches a fullscreen TUI built with [Ratatui](https://ratatui.rs/).
- [`cli/`](./cli) CLI multitool that provides the aforementioned CLIs via subcommands.

## Config

The CLI can be configured via a file named `config.toml`. By default, configuration is read from `~/.codex/config.toml`, though the `CODEX_HOME` environment variable can be used to specify a directory other than `~/.codex`.

The `config.toml` file supports the following options:

### model

The model that Codex should use.

```toml
model = "o3"  # overrides the default of "codex-mini-latest"
```

### model_provider

Codex comes bundled with a number of "model providers" predefined. This config value is a string that indicates which provider to use. You can also define your own providers via `model_providers`.

For example, if you are running ollama with Mistral locally, then you would need to add the following to your config:

```toml
model = "mistral"
model_provider = "ollama"
```

because the following definition for `ollama` is included in Codex:

```toml
[model_providers.ollama]
name = "Ollama"
base_url = "http://localhost:11434/v1"
wire_api = "chat"
```

This option defaults to `"openai"` and the corresponding provider is defined as follows:

```toml
[model_providers.openai]
name = "OpenAI"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
wire_api = "responses"
```

### model_providers

This option lets you override and amend the default set of model providers bundled with Codex. This value is a map where the key is the value to use with `model_provider` to select the correspodning provider.

For example, if you wanted to add a provider that uses the OpenAI 4o model via the chat completions API, then you

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
# valid values for wire_api are "chat" and "responses".
wire_api = "chat"
```

### approval_policy

Determines when the user should be prompted to approve whether Codex can execute a command:

```toml
# This is analogous to --suggest in the TypeScript Codex CLI
approval_policy = "unless-allow-listed"
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

### profiles

A _profile_ is a collection of configuration values that can be set together. Multiple profiles can be defined in `config.toml` and you can specify the one you
want to use at runtime via the `--profile` flag.

Here is an example of a `config.toml` that defines multiple profiles:

```toml
model = "o3"
approval_policy = "unless-allow-listed"
sandbox_permissions = ["disk-full-read-access"]
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
4. the default value that comes with Codex CLI (i.e., Codex CLI defaults to `o4-mini`)

### sandbox_permissions

List of permissions to grant to the sandbox that Codex uses to execute untrusted commands:

```toml
# This is comparable to --full-auto in the TypeScript Codex CLI, though
# specifying `disk-write-platform-global-temp-folder` adds /tmp as a writable
# folder in addition to $TMPDIR.
sandbox_permissions = [
    "disk-full-read-access",
    "disk-write-platform-user-temp-folder",
    "disk-write-platform-global-temp-folder",
    "disk-write-cwd",
]
```

To add additional writable folders, use `disk-write-folder`, which takes a parameter (this can be specified multiple times):

```toml
sandbox_permissions = [
    # ...
    "disk-write-folder=/Users/mbolin/.pyenv/shims",
]
```

### mcp_servers

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

### disable_response_storage

Currently, customers whose accounts are set to use Zero Data Retention (ZDR) must set `disable_response_storage` to `true` so that Codex uses an alternative to the Responses API that works with ZDR:

```toml
disable_response_storage = true
```

### notify

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

### history

By default, Codex CLI records messages sent to the model in `$CODEX_HOME/history.jsonl`. Note that on UNIX, the file permissions are set to `o600`, so it should only be readable and writable by the owner.

To disable this behavior, configure `[history]` as follows:

```toml
[history]
persistence = "none"  # "save-all" is the default value
```

### file_opener

Identifies the editor/URI scheme to use for hyperlinking citations in model output. If set, citations to files in the model output will be hyperlinked using the specified URI scheme so they can be ctrl/cmd-clicked from the terminal to open them.

For example, if the model output includes a reference such as `【F:/home/user/project/main.py†L42-L50】`, then this would be rewritten to link to the URI `vscode://file/home/user/project/main.py:42`.

Note this is **not** a general editor setting (like `$EDITOR`), as it only accepts a fixed set of values:

- `"vscode"` (default)
- `"vscode-insiders"`
- `"windsurf"`
- `"cursor"`
- `"none"` to explicitly disable this feature

Currently, `"vscode"` is the default, though Codex does not verify VS Code is installed. As such, `file_opener` may default to `"none"` or something else in the future.

### project_doc_max_bytes

Maximum number of bytes to read from an `AGENTS.md` file to include in the instructions sent with the first turn of a session. Defaults to 32 KiB.

### tui

Options that are specific to the TUI.

```toml
[tui]
# This will make it so that Codex does not try to process mouse events, which
# means your Terminal's native drag-to-text to text selection and copy/paste
# should work. The tradeoff is that Codex will not receive any mouse events, so
# it will not be possible to use the mouse to scroll conversation history.
#
# Note that most terminals support holding down a modifier key when using the
# mouse to support text selection. For example, even if Codex mouse capture is
# enabled (i.e., this is set to `false`), you can still hold down alt while
# dragging the mouse to select text.
disable_mouse_capture = true  # defaults to `false`
```
