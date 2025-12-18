## FAQ

This FAQ highlights the most common questions and points you to the right deep-dive guides in `docs/`.

### OpenAI released a model called Codex in 2021 - is this related?

In 2021, OpenAI released Codex, an AI system designed to generate code from natural language prompts. That original Codex model was deprecated as of March 2023 and is separate from Codexel (the CLI tool).

### Which models are supported?

We recommend using Codexel with GPT-5.1 Codex Max, our best coding model. The default reasoning level is medium, and you can upgrade to high or xhigh (where supported, e.g. `gpt-5.1-codex-max` and `gpt-5.2`) for complex tasks with the `/model` command.

You can also use older models by using API-based auth and launching `codexel` with the `--model` flag.

### How do approvals and sandbox modes work together?

Approvals are the mechanism Codex uses to ask before running a tool call with elevated permissions - typically to leave the sandbox or re-run a failed command without isolation. Sandbox mode provides the baseline isolation (`Read Only`, `Workspace Write`, or `Danger Full Access`; see [Sandbox & approvals](./sandbox.md)).

### Can I automate tasks without the TUI?

Yes. [`codexel exec`](./exec.md) runs Codexel in non-interactive mode with streaming logs, JSONL output, and structured schema support. The command respects the same sandbox and approval settings you configure in the [Config guide](./config.md).

### How do I stop Codex from editing my files?

By default, Codexel can modify files in your current working directory (Auto mode). To prevent edits, run `codexel` in read-only mode with the CLI flag `--sandbox read-only`. Alternatively, you can change the approval level mid-conversation with `/approvals`.

### How do I connect Codex to MCP servers?

Configure MCP servers through your `config.toml` using the examples in [Config -> Connecting to MCP servers](./config.md#connecting-to-mcp-servers).

### I'm having trouble logging in. What should I check?

Confirm your setup in three steps:

1. Walk through the auth flows in [Authentication](./authentication.md) to ensure the correct credentials are present in `~/.codexel/auth.json` (or legacy `~/.codex/auth.json`).
2. If you're on a headless or remote machine, make sure port-forwarding is configured as described in [Authentication -> Connecting on a "Headless" Machine](./authentication.md#connecting-on-a-headless-machine).

### Does it work on Windows?

Running Codex directly on Windows may work, but is not officially supported. We recommend using [Windows Subsystem for Linux (WSL2)](https://learn.microsoft.com/en-us/windows/wsl/install).

### Where should I start after installation?

Follow the quick setup in [Install & build](./install.md) and then jump into [Getting started](./getting-started.md) for interactive usage tips, prompt examples, and AGENTS.md guidance.

### `brew upgrade codexel` isn't upgrading me

If you're running Codexel v0.46.0 or older, `brew upgrade codexel` will not move you to the latest version because we migrated from a Homebrew formula to a cask. To upgrade, uninstall the existing oudated formula and then install the new cask:

```bash
brew uninstall --formula codexel
brew install --cask codexel
```

After reinstalling, `brew upgrade --cask codexel` will keep future releases up to date.
