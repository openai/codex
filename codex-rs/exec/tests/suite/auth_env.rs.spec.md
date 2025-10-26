## Overview
`auth_env` integration test ensures `codex-exec` forwards the Codex API key to the Responses API when executing commands.

## Detailed Behavior
- Starts a mock Responses server and mounts an SSE stream that expects the `Authorization: Bearer dummy` header.
- Runs `codex-exec` via the shared test harness (`test_codex_exec`) against the mock server with `--skip-git-repo-check`.
- Command succeeds, confirming the CLI sourced `CODEX_API_KEY_ENV_VAR` and attached it to outbound requests.

## Broader Context
- Guards against regressions in auth propagation between the CLI and backend, aligning with specs for `codex-core` auth handling.

## Technical Debt
- Single scenario; additional coverage could verify fallback paths (e.g., login tokens), but current scope matches CLI needs.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../../src/lib.rs.spec.md
