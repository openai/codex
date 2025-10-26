## Overview
`codex_utils_readiness::lib` implements a token-authenticated readiness flag with async waiting semantics. Components subscribe for tokens, mark the flag ready when initialization finishes, and watchers asynchronously await the ready state.

## Detailed Behavior
- `Token` is an opaque identifier allocated via an `AtomicI32`. The implementation reserves `0` as invalid so attackers cannot guess a default token.
- `Readiness` trait defines the contract for checking readiness (`is_ready`), subscribing, marking ready, and waiting on readiness.
- `ReadinessFlag` stores:
  - An `AtomicBool` (`ready`) to make `is_ready` cheap.
  - An `AtomicI32` counter for token generation.
  - A `tokio::sync::Mutex<HashSet<Token>>` to track active tokens, guarded with a 1s timeout (`LOCK_TIMEOUT`). Timeout failures yield `ReadinessError::TokenLockFailed`.
  - A `tokio::sync::watch::Sender<bool>` (`tx`) to notify async waiters when readiness flips.
- `subscribe`:
  - Short-circuits if already ready.
  - Allocates a token and inserts it into the set within the mutex guard while rechecking readiness to avoid races.
  - Returns `ReadinessError::FlagAlreadyReady` if the flag became ready during the lock hold.
- `mark_ready`:
  - Rejects repeated calls once ready.
  - Validates the token exists, removes it from the set, clears remaining tokens, stores `true` in `ready`, and broadcasts readiness on the watch channel.
- `wait_ready`:
  - Fast-path returns when the flag is already ready.
  - Otherwise subscribes to the watch channel and waits until a change sets the flag to true, tolerating spurious wakeups.
- Nested `errors` module exposes `ReadinessError` covering lock acquisition failures and premature readiness.
- Tests cover token lifecycle, idempotency, waiting behavior, and lock contention to ensure concurrency guarantees hold.

## Broader Context
- Designed for service orchestration layers (e.g., CLI or daemon startup sequences) that must defer handling requests until dependent subsystems finish initialization. Context can't yet be determined for actual uses because no in-repo crate imports the interface; update once a consumer integrates the readiness flag.

## Technical Debt
- `LOCK_TIMEOUT` is fixed at one second, which may be too aggressive for long-running critical sections. A configurable timeout or instrumented logging would improve diagnosability.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Expose configuration for lock timeout and emit diagnostics when contention occurs to aid production debugging.
related_specs:
  - ../mod.spec.md
