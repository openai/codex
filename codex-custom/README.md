# Codex Custom CLI (Rust Implementation)

This is a customized fork of the OpenAI Codex CLI. It builds a standalone native executable named `codex-custom` to ensure a zero-dependency install and to avoid conflicts with the official `codex` binary.

## Installing Codex Custom

Install from the Rust workspace path (this fork is not published to npm):

```shell
cargo install --path cli
codex-custom
```

You can also download a platform-specific release from the upstream [OpenAI Codex Releases](https://github.com/openai/codex/releases). Note: those artifacts are for the official CLI (`codex`). This fork builds `codex-custom` from source.

## What's new in the Rust CLI

While we are [working to close the gap between the TypeScript and Rust implementations of Codex CLI](https://github.com/openai/codex/issues/1262), note that the Rust CLI has a number of features that the TypeScript CLI does not!

### Config

Codex supports a rich set of configuration options. Note that the Rust CLI uses `config.toml` instead of `config.json`. See [`config.md`](./config.md) for details.

### Model Context Protocol Support

Codex CLI functions as an MCP client that can connect to MCP servers on startup. See the [`mcp_servers`](./config.md#mcp_servers) section in the configuration documentation for details.

It is still experimental, but you can also launch Codex as an MCP _server_ by running `codex-custom mcp`. Use the [`@modelcontextprotocol/inspector`](https://github.com/modelcontextprotocol/inspector) to try it out:

```shell
npx @modelcontextprotocol/inspector codex-custom mcp
```

### Notifications

You can enable notifications by configuring a script that is run whenever the agent finishes a turn. The [notify documentation](./config.md#notify) includes a detailed example that explains how to get desktop notifications via [terminal-notifier](https://github.com/julienXX/terminal-notifier) on macOS.

### `codex-custom exec` to run Codex programmatically/non-interactively

To run Codex non-interactively, run `codex-custom exec PROMPT` (you can also pass the prompt via `stdin`) and Codex will work on your task until it decides that it is done and exits. Output is printed to the terminal directly. You can set the `RUST_LOG` environment variable to see more about what's going on.

### Use `@` for file search

Typing `@` triggers a fuzzy-filename search over the workspace root. Use up/down to select among the results and Tab or Enter to replace the `@` with the selected path. You can use Esc to cancel the search.

### Toggle approval policy in the TUI

You can switch approval policies on-the-fly without leaving the TUI. Press `Shift+Tab` to cycle through the available policies: `untrusted`, `on-failure`, `on-request`, and `bypass approvals on` (never ask). The current workflow is shown in the composer footer as a compact status: `⏵⏵ … (Shift+Tab to cycle)`.

### `--cd`/`-C` flag

Sometimes it is not convenient to `cd` to the directory you want Codex to use as the "working root" before running Codex. Fortunately, `codex-custom` supports a `--cd` option so you can specify whatever folder you want. You can confirm that Codex is honoring `--cd` by double-checking the **workdir** it reports in the TUI at the start of a new session.

### Shell completions

Generate shell completion scripts via:

```shell
codex-custom completion bash
codex-custom completion zsh
codex-custom completion fish
```

### Experimenting with the Codex Sandbox

To test what happens when a command is run under the sandbox provided by Codex, we provide the following subcommands in this CLI:

```
# macOS
codex-custom debug seatbelt [--full-auto] [COMMAND]...

# Linux
codex-custom debug landlock [--full-auto] [COMMAND]...
```

### Selecting a sandbox policy via `--sandbox`

The Rust CLI exposes a dedicated `--sandbox` (`-s`) flag that lets you pick the sandbox policy **without** having to reach for the generic `-c/--config` option:

```shell
# Run Codex with the default, read-only sandbox
codex-custom --sandbox read-only

# Allow the agent to write within the current workspace while still blocking network access
codex-custom --sandbox workspace-write

# Danger! Disable sandboxing entirely (only do this if you are already running in a container or other isolated env)
codex-custom --sandbox danger-full-access
```

The same setting can be persisted in `~/.codex/config.toml` via the top-level `sandbox_mode = "MODE"` key, e.g. `sandbox_mode = "workspace-write"`.

### Image inputs (paste/drop)

You can attach images to a message in the TUI by pasting any of the following into the composer:

- A local image file path (absolute or relative), e.g. `/path/to/photo.jpg`
- A `file:///` URL, e.g. `file:///Users/me/screenshot.png`
- A fully qualified `http(s)://` URL to an image (png, jpg/jpeg, webp, gif)
- A base64 data URL, e.g. `data:image/png;base64,....`

When detected, the composer inserts a compact placeholder like `[image_1]` and queues the attachment to be sent with your next message. You may include multiple images; they will be sent alongside your text using OpenAI's Responses API `input_image` parts. If you delete a placeholder before sending, the corresponding attachment is removed.

You can also start a session with one or more images attached via CLI: `codex-custom -i image1.png,image2.jpg "Your prompt"`.

## Code Organization

This folder is the root of a Cargo workspace. It contains quite a bit of experimental code, but here are the key crates:

- [`core/`](./core) contains the business logic for Codex. Ultimately, we hope this to be a library crate that is generally useful for building other Rust/native applications that use Codex.
- [`exec/`](./exec) "headless" CLI for use in automation.
- [`tui/`](./tui) CLI that launches a fullscreen TUI built with [Ratatui](https://ratatui.rs/).
- [`cli/`](./cli) CLI multitool that provides the aforementioned CLIs via subcommands.
