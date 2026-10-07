# Latest model fallback

This is a compact, non-authoritative fallback, not a source for current availability, prices, aliases, or defaults. First search for and fetch current official model guidance at `https://developers.openai.com/api/docs/guides/latest-model` and the relevant official model page. The fetched official documentation wins if this snapshot has drifted. Disclose any use of this fallback.

## Source model descriptions

| Model ID | Official source description |
| --- | --- |
| `gpt-6-astra` | For the most demanding reasoning, coding, and professional work. |
| `gpt-6.1-sol` | Near-Astra performance for complex work at a lower cost. |
| `gpt-6-luna` | Strong performance for focused, high-volume tasks. |

Use `https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md#migration-quickstart` for an actual GPT-6 migration and `https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md#prompting-best-practices` for requested GPT-6 prompting. Open and read the relevant page before recommending a request shape, reasoning setting, endpoint, tool behavior, or migration.

Verify GPT-6 Pro against current official Responses and model documentation before describing model IDs, reasoning modes, request parameters, or account availability; do not invent a separate `gpt-6-pro` model slug.
