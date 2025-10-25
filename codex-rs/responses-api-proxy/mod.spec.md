## Overview
`codex-responses-api-proxy` hosts a minimal HTTP proxy that forwards `/v1/responses` requests to OpenAI. It reads an API key from stdin, binds a local listener, and relays requests with restrictive filtering so tooling can interact with the Responses API without embedding secrets.

## Detailed Behavior
- `src/lib.rs` implements the core proxy logic, including CLI parsing, key ingestion, listener startup, request forwarding, and optional shutdown handling.
- `src/read_api_task.rs` (sic) — actually `read_api_key.rs` — manages secure reading of the upstream authorization header from stdin, locking memory on Unix to avoid leaks.
- `src/main.rs` (already documented) just applies process hardening and calls `run_main`.

## Broader Context
- Used by Codex tools that need the OpenAI Responses API but run in sandboxed contexts where direct network access or environment variables are not available.

## Technical Debt
- None noted; primary TODOs (e.g., Windows `mlock` equivalent) are documented in the code comments.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/read_api_key.rs.spec.md
  - ./src/main.rs.spec.md
