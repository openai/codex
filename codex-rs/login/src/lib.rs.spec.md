## Overview
`codex_login::lib` wires the crate’s modules together and re-exports Codex auth helpers so downstream callers can access login APIs without juggling multiple imports.

## Detailed Behavior
- Declares module boundaries (`device_code_auth`, `pkce`, `server`) and re-exports their primary entrypoints:
  - `run_device_code_login` for the CLI-friendly device code experience.
  - `LoginServer`, `ServerOptions`, `ShutdownHandle`, `run_login_server` for browser-based OAuth flows.
- Re-exports high-level auth types from `codex-core` (`AuthManager`, `CodexAuth`, `AuthDotJson`, token utilities, env-var constants) plus `AuthMode` from `codex_app_server_protocol`, providing a single crate for login-centric code.

## Broader Context
- Used by the CLI and service layers to orchestrate login flows; pairing re-exports with the login server simplifies integration for new binaries wanting consistent Codex auth handling.

## Technical Debt
- None identified; as additional login helpers become available in `codex-core`, keep this module in sync.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./server.rs.spec.md
  - ./device_code_auth.rs.spec.md
  - ./pkce.rs.spec.md
