## Overview
`codex-ollama` integrates Codex with the local Ollama runtime for open-source models. It probes the local server, lists models, pulls missing ones with progress reporting, and adapts provider configuration based on whether the endpoint uses the native Ollama API or the OpenAI-compatible `/v1` API.

## Detailed Behavior
- `src/lib.rs` exposes the main `OllamaClient`, progress reporters, and `ensure_oss_ready` helper used when `--oss` is selected.
- `src/client.rs` contains the HTTP client logic for probing servers, listing models, and streaming pull events.
- `src/parser.rs`, `src/pull.rs`, and `src/url.rs` provide pull-event decoding, progress reporter implementations, and URL utilities.

## Broader Context
- Used by Codex when users opt into the built-in OSS provider (`--oss`). Ensures the local environment has the required models before the OSS workflow runs.

## Technical Debt
- None noted; future enhancements (e.g., richer TUI reporting) build on the existing progress reporter abstraction.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/client.rs.spec.md
  - ./src/parser.rs.spec.md
  - ./src/pull.rs.spec.md
  - ./src/url.rs.spec.md
