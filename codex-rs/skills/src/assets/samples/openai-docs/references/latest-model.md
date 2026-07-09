# Latest model guide

This file is a curated helper. Every recommendation here must be verified against current OpenAI docs before it is repeated to a user.

## Current model map

| Model ID | Use for |
| --- | --- |
| `gpt-5.6` | Latest/default GPT-5.6 alias; routes to Sol |
| `gpt-5.6-sol` | Flagship GPT-5.6 tier for hardest quality-first, coding, and reasoning workflows |
| `gpt-5.6-terra` | Mini-like GPT-5.6 tier for balanced cost, latency, and quality |
| `gpt-5.6-luna` | Nano-like GPT-5.6 tier for high-throughput, simple, or strict-latency tasks |
| `gpt-5.5` | Previous default text and reasoning model; use for existing GPT-5.5 integrations |
| `gpt-5.4` | Older default text and reasoning model; use for existing GPT-5.4 integrations |
| `gpt-5.4-mini` | Older lower-cost testing and lighter production workflows |
| `gpt-5.4-nano` | Older high-throughput simple tasks and classification |
| `gpt-4.1-mini` | Cheaper no-reasoning text |
| `gpt-4.1-nano` | Fastest and cheapest no-reasoning text |
| `gpt-5.3-codex` | Agentic coding, code editing, and tool-heavy coding workflows |
| `gpt-5.1-codex-mini` | Cheaper coding workflows |
| `gpt-image-2` | Best image generation and edit quality |
| `gpt-image-1.5` | Less expensive image generation and edit quality |
| `gpt-image-1-mini` | Cost-optimized image generation |
| `gpt-4o-mini-tts` | Text-to-speech |
| `gpt-4o-mini-transcribe` | Speech-to-text, fast and cost-efficient |
| `gpt-realtime-1.5` | Realtime voice and multimodal sessions |
| `gpt-realtime-mini` | Cheaper realtime sessions |
| `gpt-audio` | Chat Completions audio input and output |
| `gpt-audio-mini` | Cheaper Chat Completions audio workflows |
| `sora-2` | Faster iteration and draft video generation |
| `sora-2-pro` | Higher-quality production video |
| `omni-moderation-latest` | Text and image moderation |
| `text-embedding-3-large` | Higher-quality retrieval embeddings; default in this skill because no best-specific row exists |
| `text-embedding-3-small` | Lower-cost embeddings |

## Maintenance notes

- GPT-5.6 Pro is a Responses reasoning mode on the base model, not a separate `gpt-5.6-pro` slug. Verify the live model guide before recommending its request shape.
- This file will drift unless it is periodically re-verified against current OpenAI docs.
- If this file conflicts with current docs, the docs win.
