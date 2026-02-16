# Changelog

All notable changes to `codex-app-server-sdk` are tracked here.

## [0.1.0] - 2026-02-16

Initial SDK milestone release for Codex app-server v2.

### Added

- Core stdio JSON-RPC transport with handshake support (`initialize` + `initialized`).
- High-level API methods for:
  - `thread/start`, `thread/resume`, `thread/read`, `thread/list`
  - `turn/start`, `turn/interrupt`
  - `model/list`
- Streaming notification support via `next_notification`, `stream_until_methods`, and `wait_for_turn_completed`.
- Approval request handling for command execution and file changes.
- Notebook-friendly helpers:
  - `turn_text`, `run_text_turn`, `ask`
  - `Conversation` and `AsyncConversation` wrappers
- Async client (`AsyncAppServerClient`) with parity for core methods and helper flows.
- Typed response wrappers (`*_typed`) for core responses and common notifications.
- Schema-generated dataclass wrappers (`*_schema`) and schema generation scripts.
- Protocol `TypedDict` generation script for dict-native typing support.
- Structured JSON-RPC error mapping with overload/retry-specific exceptions.
- Retry helper with exponential backoff + jitter:
  - `retry_on_overload`
  - `request_with_retry_on_overload`

### Quality

- Added fake app-server test harness covering transport and behavior.
- Added tests ported from Rust app-server suite intent.
- Added optional real integration smoke tests (env-gated by `RUN_REAL_CODEX_TESTS=1`).
- Release sweep improvements:
  - Sync/async API parity for typed wrappers and notification parsing
  - Reduced duplicate async pathway code via centralized sync-call helper
  - Added `turn_text_typed` helper for parity with `turn_text_schema`
  - Documentation and typing consistency updates
