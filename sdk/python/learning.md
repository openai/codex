# Python SDK Learning Notes (Top SDK patterns)

This file captures patterns worth copying for a Codex app-server Python SDK.

## SDKs reviewed

1. openai/openai-python
2. stripe/stripe-python
3. boto/boto3
4. encode/httpx
5. slackapi/python-slack-sdk

## Shared patterns across strong SDKs

## 1) First-run ergonomics win adoption
- `pip install ...` then a tiny snippet that works immediately.
- Constructor defaults should be safe and obvious.
- Offer one happy-path call in README before advanced topics.

Applied here:
- `AppServerClient` can be used as a context manager in 4-5 lines.

## 2) Sync + async split is explicit
- Great SDKs usually provide both sync and async APIs or make the split very clear.
- Avoid hidden event-loop magic.

Applied here:
- v0.1 starts with a robust sync client; async wrapper planned next.

## 3) Typed surfaces and stable data contracts
- Strong SDKs expose typed request/response models, not only raw dicts.
- Backward compatibility is easier when schema versioning is explicit.

Applied here:
- v0.1 returns dicts for speed while keeping method-level contracts stable.
- next step: generated pydantic/dataclass models from app-server schema.

## 4) Error taxonomy matters
- Different error classes reduce user guesswork (`AuthError`, `RateLimitError`, `TransportError`, etc.).
- Include server payload in exceptions for debugging.

Applied here:
- `JsonRpcError`, `TransportClosedError`, and base `AppServerError`.

## 5) Escape hatches are required
- Great SDKs provide high-level APIs and low-level access for power users.

Applied here:
- High-level methods (`thread_start`, `turn_start`, ...)
- low-level primitives (`request`, `notify`, `next_notification`).

## 6) Streaming UX should be simple
- Streaming APIs should not force users to parse transport internals.
- Event iteration helpers reduce boilerplate.

Applied here:
- `wait_for_turn_completed()`
- `stream_until_methods()`

## 7) Testing strategy from mature SDKs
- Unit tests for serialization/validation.
- Integration tests using fake servers.
- “Golden path” tests for README examples.

Applied here:
- Added Python integration tests with a JSON-RPC fake app-server process.
- Ported key ideas from Rust app-server suite: initialize, thread start, turn flow, interruption.

## 8) Notebook friendliness
- Explicit helper methods reduce repetitive setup in notebooks.
- keep payload structures copy-pasteable.

Applied here:
- `turn_start(thread_id, input_items=[...])` accepts direct notebook-friendly dicts.

## 9) Versioning and compatibility policy
- Users need to know what breaks when.

Applied here:
- SDK currently targets app-server v2 method names and JSON-RPC over stdio.
- Keep compatibility matrix in README as API evolves.

## 10) Documentation structure
- Good SDK docs separate quickstart, cookbook, and API reference.

Applied here:
- README includes quickstart + event loop + notebook example.
