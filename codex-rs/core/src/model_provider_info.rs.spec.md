## Overview
`core::model_provider_info` defines how Codex describes and interacts with external model providers. It stores connection metadata (URLs, auth, headers), selects the correct wire protocol, and exposes helpers to build authenticated HTTP requests with provider-specific defaults.

## Detailed Behavior
- `WireApi` distinguishes between the OpenAI Responses (`/v1/responses`) and Chat Completions (`/v1/chat/completions`) endpoints; the enum informs payload generation across the client stack.
- `ModelProviderInfo` fields capture:
  - Display name, base URL, and optional overrides for query parameters.
  - Auth configuration (environment key, bearer token override, whether OpenAI login is required).
  - Extra headers, environment-derived headers, retry limits, and streaming timeouts.
- Key methods:
  - `create_request_builder` clones auth state (preferring explicit bearer tokens, then environment keys, falling back to ambient `CodexAuth`) and returns a `reqwest::RequestBuilder` pre-populated with auth headers and provider headers.
  - `get_full_url` chooses default endpoints (OpenAI vs ChatGPT backend) based on auth mode and appends provider-specific paths/query strings according to `wire_api`.
  - `apply_http_headers` and `api_key` resolve static and environment headers safely, surfacing detailed errors (`CodexErr::EnvVar`) when required secrets are missing.
  - Retry/timeout helpers (`request_max_retries`, `stream_max_retries`, `stream_idle_timeout`) enforce caps to prevent runaway configuration.
  - `is_azure_responses_endpoint` detects Azure-hosted Responses deployments by matching host markers or the provider name.
- Built-in providers:
  - `built_in_model_providers` registers default “openai” and “oss” providers, pulling extra headers (version, organization/project) from environment variables when present.
  - `create_oss_provider` / `create_oss_provider_with_base_url` generate a Chat-compatible provider pointing at the local OSS endpoint, defaulting to `http://localhost:11434/v1`.
- Tests exercise TOML deserialization, Azure detection heuristics, and OSS defaults.

## Broader Context
- `ModelClient` uses this struct to determine request URLs and retries; prompt builders consult `wire_api` to choose between Responses and Chat payloads.
- Configuration loaders (`config.rs`, `model_provider_info::built_in_model_providers`) merge user-specified providers with these defaults so workspace-specific overrides plug in seamlessly.
- Context can't yet be determined for future provider capabilities (e.g., streaming metadata extensions); fields like `query_params` and `http_headers` give room for incremental adoption without code changes.

## Technical Debt
- `create_request_builder` couples API key lookup, ChatGPT token handling, and HTTP header application; splitting auth resolution from request construction would clarify responsibilities and make testing individual branches easier.
- Azure detection relies on substring heuristics; sharing the logic with configuration validation (or allowing explicit provider flags) would avoid false positives/negatives as Azure expands its domains.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Extract auth resolution from `create_request_builder` so each authentication mode can be unit-tested without spinning up HTTP clients.
    - Replace heuristic Azure detection with an explicit provider flag or reusable validator to ensure future Azure domain changes are handled intentionally.
related_specs:
  - ./client.rs.spec.md
  - ./client_common.rs.spec.md
  - ./chat_completions.rs.spec.md
  - ./config.rs.spec.md
