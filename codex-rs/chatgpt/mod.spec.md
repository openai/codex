## Overview
`codex-chatgpt` bridges Codex CLI commands with the ChatGPT backend used for hosted tasks. It provides commands for fetching Codex agent tasks, converting their diff outputs into local git patches, and authenticating with stored ChatGPT tokens.

## Detailed Behavior
- Public modules:
  - `apply_command` implements the `codex chatgpt apply` CLI, wiring CLI overrides into core configuration and applying diff artifacts locally.
  - `get_task` defines the task response schema and fetches task metadata from ChatGPT using the shared HTTP client.
- Internal modules:
  - `chatgpt_client` performs authenticated GET requests against the ChatGPT backend.
  - `chatgpt_token` loads, caches, and exposes ChatGPT access tokens sourced from Codex auth storage.

## Broader Context
- Depends on `codex-core` for configuration loading, auth retrieval, and HTTP client setup. The CLI is typically invoked after Codex agents push diff artifacts to the hosted service.
- Context can't yet be determined for downstream consumers beyond the CLI entrypoint; once other crates call into this library, update references accordingly.

## Technical Debt
- Error handling mirrors the hosted API but lacks specialized retries or rate-limit awareness; CLI callers surface raw HTTP failures to the user.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Introduce retry and rate-limit handling in the ChatGPT client to improve robustness for transient backend failures.
related_specs:
  - ./src/apply_command.rs.spec.md
  - ./src/get_task.rs.spec.md
  - ./src/chatgpt_client.rs.spec.md
  - ./src/chatgpt_token.rs.spec.md
