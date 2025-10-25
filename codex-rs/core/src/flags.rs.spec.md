## Overview
`core::flags` defines environment-flag switches that let tests and local tools override runtime behavior without wiring additional configuration plumbing.

## Detailed Behavior
- Uses the `env_flags!` macro to declare `CODEX_RS_SSE_FIXTURE`, an optional path override that lets offline tests in `client.rs` replay recorded SSE traffic instead of hitting the live service.
- Defaults to `None`, so production runs ignore the flag unless the environment variable is explicitly set.

## Broader Context
- `CODEX_RS_SSE_FIXTURE` is consumed by the client execution flow (`../client.rs.spec.md`) to conditionally redirect request streams during integration testing.
- The flag integrates with the shared `env_flags` crate, keeping feature toggles consistent across the workspace.

## Technical Debt
- None; additional flags can be added alongside this definition as new test shims emerge.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../client.rs.spec.md
  - ../lib.rs.spec.md
