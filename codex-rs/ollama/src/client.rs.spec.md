## Overview
`client.rs` implements the HTTP client for interacting with a local Ollama server. It detects whether the provider uses native endpoints or the OpenAI-compatible `/v1` API, lists models, and streams pull progress events.

## Detailed Behavior
- `OllamaClient` stores a `reqwest::Client`, host root, and a flag indicating OpenAI compatibility.
- Constructors:
  - `try_from_oss_provider` reads provider settings from `Config`, builds a client, and probes the server.
  - `try_from_provider` (internal) normalizes base URLs, determines API compatibility, and calls `probe_server`.
- `probe_server` hits `/api/tags` (native) or `/v1/models` (OpenAI) and returns a user-friendly error with installation/run instructions when unreachable.
- `fetch_models` lists `models[].name` from `/api/tags`, returning an empty list if the request fails.
- `pull_model_stream` posts to `/api/pull` with `stream=true`, consumes newline-delimited JSON chunks, parses them via `pull_events_from_value`, and yields `PullEvent`s on an async stream. Terminates on `Success`, `Error`, or connection closure.
- `pull_with_reporter` drives the streaming pull with a `PullProgressReporter`, propagating errors based on events (Ollama returns HTTP 200 even when errors occur, so the stream is authoritative).
- Test helpers exist to instantiate clients against mock servers.

## Broader Context
- `ensure_oss_ready` in `lib.rs` uses `OllamaClient` to check/pull models. CLI/TUI progress reporters consume the `PullEvent` stream.

## Technical Debt
- None; error messaging already hints at installation/run steps for users.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./pull.rs.spec.md
  - ./parser.rs.spec.md
  - ./url.rs.spec.md
