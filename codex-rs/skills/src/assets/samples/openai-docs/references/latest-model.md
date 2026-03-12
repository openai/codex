# Latest model guide

This file is a curated helper. Every recommendation here must be verified against current OpenAI docs before it is repeated to a user.

## Current model map

| Model ID | Use for |
| --- | --- |
| `gpt-5.4` | Default text plus reasoning for most new apps |
| `gpt-5.4-pro` | Only when the user explicitly asks for maximum reasoning or quality; substantially slower and more expensive |
| `gpt-5-mini` | Cheaper and faster reasoning with good quality |
| `gpt-5-nano` | High-throughput simple tasks and classification |
| `gpt-5.4` | Explicit no-reasoning text path via `reasoning.effort: none` |
| `gpt-4.1-mini` | Cheaper no-reasoning text |
| `gpt-4.1-nano` | Fastest and cheapest no-reasoning text |
| `gpt-5.3-codex` | Agentic coding, code editing, and tool-heavy coding workflows |
| `gpt-5.1-codex-mini` | Cheaper coding workflows |
| `gpt-image-1.5` | Best image generation and edit quality |
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

- This file will drift unless it is periodically re-verified against current OpenAI docs.
- If this file conflicts with current docs, the docs win.
