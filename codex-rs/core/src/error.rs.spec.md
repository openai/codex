## Overview
`core::error` defines the error vocabulary for the Codex runtime. It consolidates sandbox, IO, HTTP, and quota failures into a single `CodexErr` enum so higher layers (CLI, TUI, unified exec) can handle failures consistently and produce user-friendly messages.

## Detailed Behavior
- Introduces `SandboxErr` for sandbox-level issues, distinguishing access denials, timeouts, signals, seccomp/landlock failures, and missing sandbox binaries.
- The main `CodexErr` enum covers:
  - Control-flow conditions (`TurnAborted`, `Stream`, `RetryLimit`, `InternalAgentDied`) and conversation lifecycle issues (`ConversationNotFound`, `SessionConfiguredNotFirstEvent`).
  - Model/context problems (`ContextWindowExceeded`, `UsageNotIncluded`), command execution errors (`Timeout`, `Spawn`, `Interrupted`), and HTTP transport failures (`UnexpectedStatus`, `UsageLimitReached`, `ResponseStreamFailed`, `ConnectionFailed`).
  - Fatal or unsupported operations plus automatic `From` conversions for `io::Error`, `serde_json::Error`, tokio join errors, landlock setup errors (Linux), and `CancelErr`.
- Wrapper structs (`ConnectionFailedError`, `ResponseStreamFailed`, `UnexpectedResponseError`, `RetryLimitReachedError`, `UsageLimitReachedError`, `EnvVarError`) encapsulate additional metadata (HTTP status, request IDs, plan types, guidance text) and format descriptive messages for downstream display.
- Helper functions:
  - `get_error_message_ui` truncates verbose sandbox output (via `truncate_middle`) while preserving helpful context; special-cases sandbox denials and timeouts.
  - `retry_suffix`, `retry_suffix_after_or`, `remaining_seconds`, and `format_reset_duration` compute human-readable retry windows, honoring a test-only `NOW_OVERRIDE`.
  - `CodexErr::downcast_ref` maintains compatibility with previous `anyhow`-based callers by exposing dynamic downcasting on the concrete enum.
- Unit tests validate string formatting across plan types, retry windows, and timeout messaging to guard against regressions in end-user copy.

## Broader Context
- Consumed by most core services (`../../lib.rs.spec.md`, `../client.rs.spec.md`, unified exec) to propagate failures up to UI layers with consistent semantics.
- `UsageLimitReachedError` relies on quota data from `token_data` and `RateLimitSnapshot` to steer messaging; downstream UIs surface those messages directly.
- Sandboxing-related variants coordinate with `../exec.rs.spec.md`, `../seatbelt.rs.spec.md`, and the command safety suite to keep execution guardrails aligned.

## Technical Debt
- None noted; error coverage and formatting helpers align with current feature set.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../lib.rs.spec.md
  - ../exec.rs.spec.md
  - ../token_data.rs.spec.md
  - ../truncate.rs.spec.md
