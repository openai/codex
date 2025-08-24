## FAQ

### OpenAI released a model called Codex in 2021 - is this related?

In 2021, OpenAI released Codex, an AI system designed to generate code from natural language prompts. That original Codex model was deprecated as of March 2023 and is separate from the CLI tool.

### Which models are supported?

We recommend using gpt-5. However, any model available with the [Responses API](https://platform.openai.com/docs/api-reference/responses) is supported. Use the `--model` flag to specify the model you want to use.

### Why does `o3` or `o4-mini` not work for me?

It's possible that your [API account needs to be verified](https://help.openai.com/en/articles/10910291-api-organization-verification) in order to start streaming responses and seeing chain of thought summaries from the API. If you're still running into issues, please let us know!

### How do I stop Codex from editing my files?

By default, Codex can modify files in your current working directory (Auto mode). To prevent edits, run `codex` in read-only mode with the CLI flag `--sandbox read-only`. Alternatively, you can change the approval level mid-conversation with `/approvals`.

### Does it work on Windows?

Running Codex directly on Windows may work, but is not officially supported. We recommend using [Windows Subsystem for Linux (WSL2)](https://learn.microsoft.com/en-us/windows/wsl/install). 