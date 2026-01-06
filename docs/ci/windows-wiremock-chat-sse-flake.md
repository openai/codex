# Windows CI flake: WireMock + Chat Completions SSE never completes

## Summary

Some Windows CI runs intermittently failed `codex-app-server` integration tests that use a WireMock “mock model server” for the legacy Chat Completions streaming API (`/v1/chat/completions`).

The failure mode looked like:

- WireMock verification panic on drop (e.g. “expected 2/4 matching requests, got 1”).
- The single observed request was `POST /v1/chat/completions` returning an SSE stream that included tool calls.
- The tests expected multiple model calls across a conversation/turn sequence, but Codex never issued the follow-up request.

## Root cause

The mock server responds with an SSE payload that contains:

- a JSON `data: ...` event whose `finish_reason` is `"tool_calls"` (or `"stop"`), and
- a terminal sentinel event `data: DONE` (historical test fixture) or `data: [DONE]` (upstream convention).

On Windows (notably under WireMock), the HTTP connection can remain open after emitting a `finish_reason`-bearing JSON event, and the sentinel may not be observed in a way that closes the stream promptly from the client’s perspective.

Prior to the fix, our Chat Completions SSE parsing treated `finish_reason == "tool_calls"` as “emit tool call items”, but it did **not** unconditionally emit `ResponseEvent::Completed` / terminate the stream at that point. That meant:

1. Codex received tool calls and began executing/handling them.
2. The core turn runner (`try_run_turn`) continued waiting for `ResponseEvent::Completed` to mark the turn finished.
3. Since completion never arrived, the turn stalled, no follow-up model request was issued, and WireMock later panicked because it only saw the first request.

This is why the failures were “flaky”: it depended on timing/stream termination behavior on Windows rather than deterministic application logic.

## Fix

We changed the Chat Completions SSE parser to treat `finish_reason` as authoritative for end-of-response:

- When a choice reports `finish_reason == "tool_calls"`, we:
  - flush any accumulated reasoning item (if present),
  - emit the `FunctionCall` response items,
  - emit `ResponseEvent::Completed`, and
  - return from the SSE processing loop.
- When a choice reports `finish_reason == "stop"`, we flush any accumulated reasoning/assistant items, emit `Completed`, and return.

We still accept `data: DONE` / `data: [DONE]` as a terminal sentinel for compatibility, but the parser no longer depends on that sentinel to complete a turn.

Implementation: `codex-rs/codex-api/src/sse/chat.rs`

## Protocol correctness and risk

This change was motivated by flaky tests, but it is not a test-only workaround; it corrects completion semantics in the Chat Completions streaming handler.

In the Chat Completions stream, `finish_reason` in the JSON payload (`"tool_calls"`, `"stop"`, etc.) is the semantic signal that the model is done producing output for the current response. The `DONE`/`[DONE]` line is a transport sentinel that many servers emit, but clients should not rely on it being observed promptly (or at all) to make forward progress—especially in environments where the HTTP connection may remain open.

To keep risk low:
- We only change behavior when we see an explicit `finish_reason` value that already indicates the stream is complete; in those cases we now emit `ResponseEvent::Completed` immediately instead of waiting for a sentinel/close.
- We preserve compatibility by still accepting `DONE`/`[DONE]` as a completion signal.
- We validate the behavior with focused unit tests in `codex-api` plus the higher-level `codex-app-server` integration tests that were flaking.

## Why this fixes the tests

The integration tests in `codex-app-server` rely on turn completion to progress the conversation and trigger subsequent model calls. By guaranteeing that the SSE parser emits a `Completed` event as soon as the model indicates it is finished (via `finish_reason`), the core state machine can:

- complete the current turn deterministically,
- proceed to the next turn/model request immediately, and
- satisfy WireMock’s expected request count reliably on Windows.

## Verification

Locally, we validated:

- `cargo test -p codex-api --lib`
- `cargo test -p codex-app-server codex_message_processor_flow`

The previously failing `codex_message_processor_flow` tests now pass.

In particular, `codex-api` includes regression tests that simulate a stream that *does not close* after emitting a JSON event with `finish_reason` (to mimic WireMock/Windows behavior). Those tests ensure we emit `Completed` and return without relying on `DONE`/connection close.
