# Latest model fallback

This is a compact, non-authoritative fallback, not a source for current availability, prices, aliases, or defaults. First search for and fetch current official model guidance at `https://developers.openai.com/api/docs/guides/latest-model` and the relevant official model page. The fetched official documentation wins if this snapshot has drifted. Disclose any use of this fallback.

## Source model descriptions

| Model ID | Official source description |
| --- | --- |
| `gpt-6.2-astra` | Demanding reasoning and coding tasks. |
| `gpt-6.2-sol` | Everyday coding and agent workflows. |
| `gpt-6.1-terra` | Mini model for latency-sensitive API examples. |
| `gpt-6-luna` | Nano model for focused, high-volume tasks. |

Legacy Chat Completions function-tool examples use `gpt-5.6-terra`. For an explicit GPT-6.2 Astra or Sol target, use the [GPT-6.2 migration quickstart](https://developers.openai.com/api/docs/guides/latest-model?model=gpt-6.2-astra#gpt-6.2-astra-migration-quickstart). Both models support Responses and Chat Completions. Tool calling requires Responses.

Codex CLI examples use `gpt-6.2-sol` when the account has access. Configuration examples shared with the IDE extension use `gpt-6.1-sol`. Check [Codex availability by plan](https://developers.openai.com/codex/pricing#feature-availability) for the user's surface.

Verify GPT-6 Pro against current official Responses and model documentation before describing model IDs, reasoning modes, request parameters, or account availability; do not invent a separate `gpt-6-pro` model slug.
