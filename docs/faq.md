## FAQ

### OpenAI released a model called Codex in 2021 - is this related?

In 2021, OpenAI released Codex, an AI system designed to generate code from natural language prompts. That original Codex model was deprecated as of March 2023 and is separate from the CLI tool.

### Which models are supported?

We recommend using Codex with GPT-5, our best coding model. The default reasoning level is medium, and you can upgrade to high for complex tasks with the `/model` command.

You can also use older models by using API-based auth and launching codex with the `--model` flag.

### Why does `o3` or `o4-mini` not work for me?

It's possible that your [API account needs to be verified](https://help.openai.com/en/articles/10910291-api-organization-verification) in order to start streaming responses and seeing chain of thought summaries from the API. If you're still running into issues, please let us know!

### How do I stop Codex from editing my files?

By default, Codex can modify files in your current working directory (Auto mode). To prevent edits, run `codex` in read-only mode with the CLI flag `--sandbox read-only`. Alternatively, you can change the approval level mid-conversation with `/approvals`.

### Does it work on Windows?

Running Codex directly on Windows may work, but is not officially supported. We recommend using [Windows Subsystem for Linux (WSL2)](https://learn.microsoft.com/en-us/windows/wsl/install).

### Why does `/status` show 272k context window when the platform docs say 400k?

The `/status` command shows the **input** context window (272,000 tokens for GPT-5-Codex), which is the maximum size for your prompts, conversation history, and context.

GPT-5-Codex has a separate **output** token limit of 128,000 tokens for responses. The total token budget is 400,000 tokens (272k input + 128k output), which is what the [platform documentation](https://platform.openai.com/docs/models/gpt-5-codex) refers to.

See [`model_context_window`](./config.md#model_context_window) and [`model_max_output_tokens`](./config.md#model_max_output_tokens) in the configuration docs for more details.
