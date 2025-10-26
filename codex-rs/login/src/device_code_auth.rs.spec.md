## Overview
`device_code_auth` implements the Codex device-code login flow for environments where launching a browser is impractical. It handles user-code retrieval, polling for authorization, PKCE coordination, and token persistence.

## Detailed Behavior
- Defines request/response structs for the device auth API (`UserCodeReq`, `UserCodeResp`, `TokenPollReq`, `CodeSuccessResp`) and a helper to deserialize string-encoded polling intervals.
- `request_user_code` posts to `{auth_base_url}/deviceauth/usercode`, handling 404s with a friendly error when device login is disabled. Successful responses return the device auth ID, user code, and poll interval.
- `poll_for_token` loops until an authorization code is issued or a 15-minute timeout elapses. It sleeps for the server-provided interval between `403`/`404` responses, aborting on other HTTP errors.
- `print_colored_warning_device_code` displays safety guidance (ANSI yellow/bold) before revealing the user code.
- `run_device_code_login(opts)` orchestrates the full flow:
  - Creates an HTTP client and normalizes issuer URLs.
  - Prints instructions directing the user to `https://auth.openai.com/codex/device`.
  - Polls for the authorization code, constructs PKCE verifier/challenge pairs, and exchanges the code via `server::exchange_code_for_tokens`.
  - Enforces workspace restrictions via `ensure_workspace_allowed` and persists tokens asynchronously using `server::persist_tokens_async`.
  - Propagates errors (e.g., workspace mismatch, persistence failure) as `io::Error` instances so CLI callers can surface actionable messages.

## Broader Context
- Invoked by the CLI when `--device-code` is requested or when headless environments cannot open a browser. Shares PKCE generation and token persistence with the primary browser-based flow in `server.rs`.

## Technical Debt
- Polling uses fixed-interval sleeps without jitter or exponential backoff; adding resilience to server throttling could improve UX on slow networks.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add jitter/backoff when polling the token endpoint to reduce load and tolerate throttled responses.
related_specs:
  - ../mod.spec.md
  - ./pkce.rs.spec.md
  - ./server.rs.spec.md
