# Latest model fallback

This is a compact, non-authoritative fallback, not a source for current availability, prices, aliases, or defaults. First search for and fetch current official model guidance at `https://developers.openai.com/api/docs/guides/latest-model` and the relevant official model page. The fetched official documentation wins if this snapshot has drifted. Disclose any use of this fallback.

## GPT-5.6 family

| Model ID | Documented workload to verify against the current model page |
| --- | --- |
| `gpt-5.6` | GPT-5.6 family alias; verify its currently documented routing and availability. |
| `gpt-5.6-sol` | Quality-first flagship, reasoning, and difficult coding work. |
| `gpt-5.6-terra` | Balanced quality, latency, and cost. |
| `gpt-5.6-luna` | High-throughput, lower-latency work. |

Use `https://developers.openai.com/api/docs/guides/upgrading-to-gpt-5p6-sol` for an actual GPT-5.6 migration and `https://developers.openai.com/api/docs/guides/prompt-guidance-gpt-5p6` for requested GPT-5.6 prompting. Open and read the relevant page before recommending a request shape, reasoning setting, endpoint, tool behavior, or migration.

## Explicitly requested existing models

| Model ID | Boundary |
| --- | --- |
| `gpt-4.1` | Preserve only when the user explicitly requests this model or existing migration target; search and fetch its own current official guide. |
| `gpt-5.4` | Preserve only when the user explicitly requests this model or existing migration target; search and fetch its own current official guide. |

Do not promote a legacy model as the current default, substitute it into an unrelated task, or replace an explicitly requested legacy target with GPT-5.6. Recommend a specialized image, audio, realtime, coding, moderation, or embedding model only after verifying the requested modality against current official documentation.

Verify GPT-5.6 Pro against current official Responses and model documentation before describing model IDs, reasoning modes, request parameters, or account availability; do not invent a separate `gpt-5.6-pro` model slug.
