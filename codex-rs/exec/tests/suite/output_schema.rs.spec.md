## Overview
`output_schema` verifies that `codex-exec` sends user-supplied JSON schemas in the request payload when invoking the Responses API.

## Detailed Behavior
- Writes a JSON schema to disk, passes `--output-schema <path>` to `codex-exec`, and runs against a mock Responses server.
- After the command succeeds, inspects the captured request body to ensure `text.format` includes the expected schema descriptor (`type: json_schema`, `name: codex_output_schema`, `strict: true`, with the schema contents).

## Broader Context
- Guards the contract between CLI flags and backend request formatting, complementing specs in `codex-core` around format payloads.

## Technical Debt
- Single scenario focusing on the happy path; additional tests could cover invalid schema files or strictness toggles if those become configurable.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../../src/lib.rs.spec.md
