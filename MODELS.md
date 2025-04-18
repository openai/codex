# Models in Codex CLI

## Default Models

- **Agentic Mode** (interactive/code-assistant):
  - Default: `o4-mini`
  - Alternate: `o3` for faster, lower‑cost responses at slightly reduced quality.

- **Full‑Context Mode** (`--full-context`):
  - Default: `gpt-4.1`

These defaults can be overridden per‑run or set permanently in your config.

## Supported Models and Limitations

Codex CLI leverages OpenAI’s `/responses` endpoint. You may use any model in your OpenAI account that supports this endpoint, subject to:

- **API Access**: The model must appear in the list from:
  ```bash
  openai models list
  ```
- **Recommended List**: The CLI preloads a short list of fast defaults: `o4-mini`, `o3`. Models not in this list or the API response will trigger a startup warning.
- **Context & Token Limits**: Each model has a maximum context window and rate/usage quotas.

## Choosing the Right Model

- **`o3`**: Fastest & cheapest, lower quality.
- **`o4-mini`**: Balanced default for speed, cost, and quality.
- **GPT‑4 family** (`gpt-4`, `gpt-4.1`, `gpt-4o`): Higher quality, larger context, higher cost.

If you hit a `max_tokens` error, either shorten your prompt/history or switch to a model with a larger context window.

## Experimenting with New Models

### One‑Off Overrides

Use the `--model` (or `-m`) flag:

```bash
codex --model gpt-4 "Refactor this function to handle errors gracefully"
```

### Persistent Default

Edit your config at `~/.codex/config.json` (or `.yaml`):

```json
{
  "model": "gpt-4"
}
```

Next runs of `codex` will default to your chosen model.

## Troubleshooting

- **Unknown Model**: Verify spelling and availability with `openai models list`.
- **Missing API Key**: Ensure `OPENAI_API_KEY` is set in your environment.
- **Rate Limits / Quotas**: Check your OpenAI dashboard or switch to a less restrictive model.

---
*For more info, refer to the OpenAI API docs: https://platform.openai.com/docs/models*