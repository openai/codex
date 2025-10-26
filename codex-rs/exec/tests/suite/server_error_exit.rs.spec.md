## Overview
`server_error_exit` ensures `codex-exec` exits non-zero when the Responses API reports a failure event.

## Detailed Behavior
- Mounts an SSE stream containing a single `response.failed` event with a synthetic error.
- Runs `codex-exec --experimental-json` and verifies it exits with status `1`, signaling automation should treat the run as failed.

## Broader Context
- Guards CLI error-propagation behavior so scripts can detect server failures.

## Technical Debt
- Single scenario focused on JSON output mode; additional coverage could assert traditional text mode mirrors the same exit behavior.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../../src/lib.rs.spec.md
