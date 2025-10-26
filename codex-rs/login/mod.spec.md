## Overview
`codex-login` hosts the browser-based and device-code authentication flows that power the `codex login` CLI. It coordinates local OAuth callbacks, PKCE generation, device-code polling, and persistence of Codex auth state.

## Detailed Behavior
- Re-exports the login server types (`LoginServer`, `ServerOptions`, `ShutdownHandle`, `run_login_server`) and the device-code flow entrypoint (`run_device_code_login`) defined under `src/server.rs` and `src/device_code_auth.rs`.
- Surfaces commonly used auth helpers from `codex-core` (e.g., `CodexAuth`, `AuthManager`, token and auth file utilities) so callers can depend on a single crate for login orchestration.

## Broader Context
- CLI tooling invokes these surfaces when handling `codex login` workflows. Service entrypoints that need to embed Codex login flows can reuse the same crate for consistency with the CLI.

## Technical Debt
- None noted at the crate level; future additions should update the spec as new login modalities appear.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/server.rs.spec.md
  - ./src/device_code_auth.rs.spec.md
  - ./src/pkce.rs.spec.md
