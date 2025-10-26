## Overview
`chatgpt_client` houses the HTTP helper used to contact the ChatGPT backend with Codex-managed credentials.

## Detailed Behavior
- `chatgpt_get_request(config, path)`:
  - Reads the base URL from `Config::chatgpt_base_url`.
  - Ensures the ChatGPT token cache is populated via `init_chatgpt_token_from_auth`.
  - Instantiates the shared reqwest client (`codex_core::default_client::create_client`).
  - Fetches the cached `TokenData`, ensuring both the access token and `account_id` are present before issuing the request.
  - Performs a GET with bearer auth and a `chatgpt-account-id` header required by the backend.
  - On success, deserializes the JSON into the caller’s type; otherwise, returns a contextualized error with HTTP status and body text.
- Uses `anyhow::Context` heavily to annotate failures, aiding CLI diagnostics.

## Broader Context
- Shared by `get_task` (for `/wham/tasks/{task_id}`) and ready for future endpoints that need the same authentication flow.
- Relies on `chatgpt_token` to source credentials from `codex-core` auth state, keeping token parsing logic centralized.

## Technical Debt
- Lacks retry, timeout tuning, or structured error codes; transient network issues result in immediate CLI failures.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add configurable timeouts/retries and map HTTP errors into actionable codes for CLI presentation.
related_specs:
  - ../mod.spec.md
  - ./chatgpt_token.rs.spec.md
  - ./get_task.rs.spec.md
