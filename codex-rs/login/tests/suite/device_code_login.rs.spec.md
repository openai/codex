## Overview
`device_code_login` exercises the device-code authentication flow against a mocked Codex auth backend. It validates success, error, timeout, workspace restrictions, and persistence behaviors.

## Detailed Behavior
- Helper functions build JWTs, seed `/deviceauth/usercode` and `/deviceauth/token` endpoints via WireMock, and craft OAuth token responses or errors.
- `device_code_login_integration_succeeds` walks the happy path: two-step polling (first 404, then success), mock token exchange, and verification that `auth.json` is persisted with tokens and matching workspace.
- Failure scenarios include:
  - Workspace mismatch returning `PermissionDenied` and leaving `auth.json` absent.
  - `/deviceauth/usercode` failures bubbling up descriptive errors.
  - Token exchange success without API key generation still persisting tokens (with `openai_api_key` unset).
  - Error payloads from `/deviceauth/token` ensuring the flow aborts without writing auth state.
- All tests honor `skip_if_no_network!` so they don’t run under sandboxes lacking outbound network mocks.

## Broader Context
- Complements `src/device_code_auth.rs`, ensuring regressions in polling or token persistence are caught before shipping.
- Shares SSE/mock conventions with other integration suites, leveraging `WireMock` and temp Codex homes.

## Technical Debt
- Repeated WireMock setup helpers duplicate patterns found in other suites; extracting reusable fixtures could simplify future maintenance.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Consolidate WireMock helpers for device-code tests into shared fixtures to reduce duplication across login and app-server suites.
related_specs:
  - ../../mod.spec.md
  - ../mod.spec.md
  - ../../src/device_code_auth.rs.spec.md
