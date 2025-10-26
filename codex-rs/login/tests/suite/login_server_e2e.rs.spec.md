## Overview
`login_server_e2e` validates the browser-based login server against a bespoke mock issuer. It focuses on end-to-end behaviors such as token persistence, workspace restrictions, port reuse, and server cancellation.

## Detailed Behavior
- `start_mock_issuer` spins up a `tiny_http` server that responds to `/oauth/token` with deterministic JWTs containing configurable ChatGPT account IDs.
- `end_to_end_login_flow_persists_auth_json` seeds a stale `auth.json`, runs the login server with a forced state/workspace ID, simulates the OAuth redirect, and asserts that the new tokens overwrite the old state.
- `forced_chatgpt_workspace_id_mismatch_blocks_login` ensures mismatched workspace IDs surface a permission error, leaving `auth.json` untouched.
- `cancels_previous_login_server_when_port_is_in_use` verifies that launching a second server on the same port triggers the first server’s `/cancel` path and that both servers report interruption.
- All tests use `skip_if_no_network!` to avoid running under restricted sandboxes and rely on temp directories for hermetic Codex homes.

## Broader Context
- Provides integration coverage for `src/server.rs`, confirming the manual `tiny_http` response handling, workspace enforcement, and cancellation behavior introduced by production login flows.

## Technical Debt
- Mock issuer duplicates logic from the actual server (manual JWT crafting, token responses); a shared helper would ease future maintenance.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Extract common mock issuer utilities to shared test support so future login tests and other crates can reuse them.
related_specs:
  - ../../mod.spec.md
  - ../mod.spec.md
  - ../../src/server.rs.spec.md
