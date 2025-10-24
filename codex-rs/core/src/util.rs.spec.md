## Overview
`core::util` currently exposes a single `backoff` helper used across networking code. It computes an exponential delay with jitter so repeated retries avoid thundering-herd effects.

## Detailed Behavior
- `backoff(attempt: u64)` treats attempts as 1-indexed, using:
  - Base delay `INITIAL_DELAY_MS` (200 ms).
  - Multiplicative factor `BACKOFF_FACTOR` (2× per attempt after the first).
  - Random jitter from `rand::rng().random_range(0.9..1.1)` to stagger concurrent retries.
- The resulting delay is returned as a `Duration` and is consumed by `ModelClient` and other networked components.

## Broader Context
- Client modules (`client.rs`, `chat_completions.rs`) call this helper when handling retryable HTTP or transport errors. Having a single implementation prevents divergence between pipelines.
- Context can't yet be determined for jitter configuration; future use cases (e.g., deterministic tests) may require hooks for overriding randomness.

## Technical Debt
- The helper relies on a global RNG; providing a deterministic path or injecting a seeded RNG would make retry logic more testable.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Allow callers to inject or override the jitter RNG for deterministic testing or custom retry strategies.
related_specs:
  - ./client.rs.spec.md
  - ./chat_completions.rs.spec.md
