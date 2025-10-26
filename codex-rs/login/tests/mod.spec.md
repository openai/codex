## Overview
`login/tests` exercises the Codex login flows end to end. The integration suite lives under `tests/suite` and is compiled as a single binary via `tests/all.rs`.

## Detailed Behavior
- `all.rs` pulls the `suite` module into one binary, mirroring the project-wide convention for aggregating async integration tests.
- `suite` contains:
  - `device_code_login.rs`, which validates device-code authentication against mocked API endpoints.
  - `login_server_e2e.rs`, which runs the local OAuth callback server against a fake issuer.

## Broader Context
- Tests depend on `core_test_support` skip macros to avoid running in network-restricted sandboxes, aligning with other workspace integration suites.

## Technical Debt
- None noted; the suite is organized similarly to other integration harnesses and follows the project’s shared testing conventions.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./all.rs.spec.md
  - ./suite/mod.rs.spec.md
