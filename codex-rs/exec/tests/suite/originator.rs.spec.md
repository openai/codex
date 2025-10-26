## Overview
`originator` ensures `codex-exec` sets the correct `Originator` header when calling the Responses API and honors internal overrides.

## Detailed Behavior
- `send_codex_exec_originator` runs `codex-exec` against a mock Responses server that expects `Originator: codex_exec`.
- `supports_originator_override` sets `CODEX_INTERNAL_ORIGINATOR_OVERRIDE` and verifies the header reflects the override value.
- Both scenarios stream simple assistant responses to keep focus on header validation.

## Broader Context
- Protects telemetry and analytics that rely on originator metadata, aligning with CLI specs around request shaping.

## Technical Debt
- Tests only cover success cases; future additions could assert behavior when overrides are invalid or missing.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../../src/lib.rs.spec.md
