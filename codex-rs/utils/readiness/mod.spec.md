## Overview
`codex-utils-readiness` offers a token-gated readiness flag that lets async tasks coordinate startup sequencing. Callers subscribe to receive authorization tokens and later mark the system ready once prerequisites complete.

## Detailed Behavior
- Re-exports the `Readiness` trait, `ReadinessFlag` implementation, and `Token` type from `src/lib.rs`.
- Keeps error types scoped within the crate (`errors::ReadinessError`) to avoid leaking internal concurrency semantics to dependents.

## Broader Context
- Built for services that need to block user-facing features until background initialization completes (e.g., database migrations, credential provisioning). Context can't yet be determined for concrete consumers because no current crates import it; document integrations once adopted.

## Technical Debt
- Token lock timeouts are hard-coded to one second; the crate lacks configuration hooks for workloads that need longer critical sections.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Allow customization of the mutex lock timeout so longer setup tasks do not spuriously fail.
related_specs:
  - ./src/lib.rs.spec.md
