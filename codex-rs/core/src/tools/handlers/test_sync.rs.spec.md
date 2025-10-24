## Overview
`core::tools::handlers::test_sync` is an internal coordination tool used to exercise concurrent tool execution. It provides sleep hooks and barrier synchronization so automated tests can orchestrate parallel calls deterministically.

## Detailed Behavior
- Accepts `ToolPayload::Function` parsed into `TestSyncArgs`, which may include optional `sleep_before_ms`, `sleep_after_ms`, and an optional barrier configuration.
- Sleep phases use `tokio::time::sleep` to delay the task before or after barrier coordination. Zero or missing values skip the delay.
- Barrier handling (`wait_on_barrier`) uses a global `OnceLock` storing a `HashMap<String, BarrierState>` guarded by an async mutex. For each barrier ID:
  - If the barrier exists, ensures the participant count matches; otherwise, returns an error.
  - Creates a new `tokio::sync::Barrier` when first seen.
  - Waits with a timeout (default 1 second); timeouts or zero participants produce model-facing errors. The leader removes the barrier record after completion so future runs can reuse the ID.
- Successful runs return `ToolOutput::Function { content: "ok", success: Some(true) }`, allowing tests to assert completion.

## Broader Context
- Registered only when experimental tools include `test_sync_tool`. It is not intended for production use but helps verify task orchestration (`parallel.rs`) and approval flows under concurrency.
- Because the handler stores global state, tests must ensure unique barrier IDs to avoid collisions across suites.
- Context can't yet be determined for multiple simultaneous barriers with the same ID; current design assumes deterministically managed IDs by tests.

## Technical Debt
- None identified; functionality is intentionally narrow for test scenarios.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.rs.spec.md
  - ../spec.rs.spec.md
  - ../../parallel.rs.spec.md
