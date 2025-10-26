## Overview
`env_filter` verifies the mock cloud tasks backend returns environment-specific task lists, ensuring the mock client used in tests matches production expectations.

## Detailed Behavior
- Creates a `MockClient` and calls `CloudBackend::list_tasks` three times:
  - Without an environment filter, expecting default tasks that include “Update README”.
  - With `env-A`, asserting only one task is returned.
  - With `env-B`, asserting two tasks are returned with the expected prefix.
- Confirms the mock backend honors the environment parameter, which other integration tests rely on for deterministic fixtures.

## Broader Context
- Provides regression coverage for the mock backend documented in Phase 3B, ensuring environment filtering remains consistent when the mock data changes.

## Technical Debt
- Test is intentionally lightweight; no additional debt identified.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../src/lib.rs.spec.md
