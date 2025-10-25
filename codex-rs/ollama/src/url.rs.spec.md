## Overview
`url.rs` contains URL helpers for the Ollama integration. It detects whether a provider base URL points to the OpenAI-compatible `/v1` API and extracts the host root used for native Ollama endpoints.

## Detailed Behavior
- `is_openai_compatible_base_url` checks whether the trimmed base URL ends with `/v1`.
- `base_url_to_host_root` removes the `/v1` suffix (if present) and trailing slashes, returning the root host URL used for native API calls.
- Unit tests cover various trailing-slash combinations.

## Broader Context
- `OllamaClient` uses these helpers to normalize endpoints before probing servers and issuing requests, ensuring the client works with both native and OpenAI-compatible modes.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./client.rs.spec.md
