# Codex App Server Python SDK (v2)

Python SDK for `codex app-server` JSON-RPC v2 over stdio.

## Status

- ✅ initialize + initialized handshake
- ✅ core thread/turn methods (`thread/*`, `turn/*`, `model/list`)
- ✅ extended thread lifecycle + control methods (`thread/fork`, `thread/archive`, `thread/unarchive`, `thread/setName`, `turn/steer`)
- ✅ streaming notification consumption
- ✅ command/file approval request handling
- ✅ async client (`AsyncAppServerClient`)
- ✅ fluent thread abstraction (`Conversation` / `AsyncConversation`)
- ✅ schema-backed typed response helpers (`*_schema`)
- ✅ lightweight typed wrappers (`*_typed`) for core responses + common notifications (sync + async parity)
- ✅ robust JSON-RPC error mapping (standard and server-error codes)
- ✅ overload retry helper (`request_with_retry_on_overload`, `retry_on_overload`)
- ✅ fake + optional real integration tests (env-gated)

## Install

```bash
cd sdk/python
python -m pip install -e .
```

For local development/tests:

```bash
python -m pip install -e '.[dev]'
```

## Quickstart

```python
from codex_app_server import AppServerClient

with AppServerClient() as client:
    client.initialize()

    thread = client.thread_start(model="gpt-5")
    thread_id = thread["thread"]["id"]

    turn = client.turn_text(thread_id, "Explain Newton's method in 3 bullets")
    turn_id = turn["turn"]["id"]

    final = client.wait_for_turn_completed(turn_id)
    print(final.method, final.params["turn"]["status"])
```

## Typed wrappers

Use typed wrappers if you want dataclass ergonomics without giving up dict-native APIs:

```python
with AppServerClient() as client:
    client.initialize()

    started = client.thread_start_typed(model="gpt-5")
    resumed = client.thread_resume_typed(started.thread.id)
    forked = client.thread_fork_typed(started.thread.id)
    _ = client.thread_archive_typed(started.thread.id)
    _ = client.thread_unarchive_typed(started.thread.id)
    _ = client.thread_set_name_typed(started.thread.id, "My Thread")
    listed = client.thread_list_typed(limit=10)
    models = client.model_list_typed()
    turn = client.turn_text_typed(started.thread.id, "hello")
    steered = client.turn_steer_typed(started.thread.id, turn.turn.id, "continue")
```

Schema wrappers (generated from protocol schemas):

```python
started = client.thread_start_schema(model="gpt-5")
turn = client.turn_text_schema(started.thread.id, "hello")
print(turn.turn.status)
```

## Error handling + retry

JSON-RPC failures are mapped to richer error classes:

- `ParseError`, `InvalidRequestError`, `MethodNotFoundError`, `InvalidParamsError`, `InternalRpcError`
- `ServerBusyError` / `RetryLimitExceededError` for overload-style server errors

```python
from codex_app_server import AppServerClient, ServerBusyError

with AppServerClient() as client:
    client.initialize()
    try:
        out = client.request_with_retry_on_overload("some/method", {"x": 1}, max_attempts=4)
    except ServerBusyError:
        # exhausted retries
        ...
```

## Notification parsing helpers

```python
evt = client.next_notification()
typed = client.parse_notification_typed(evt)
schema = client.parse_notification_schema(evt)
```

Covers common notifications such as:

- `thread/started`
- `thread/nameUpdated`
- `thread/tokenUsageUpdated`
- `turn/started`
- `turn/completed`
- `item/started`
- `item/completed`
- `item/agentMessage/delta`
- `error`

## Migration notes (earlier SDK builds → current)

- Existing dict-returning methods are unchanged.
- Prefer `turn_text(...)` over manually constructing `input=[{"type": "text", ...}]` for text-only turns.
- New typed helpers are additive (`*_typed`, `*_schema`) and safe to adopt incrementally.
- Error handling is stricter: JSON-RPC responses now raise mapped subclasses instead of only raw `JsonRpcError`.
- For transient overloads, switch ad-hoc retry loops to `request_with_retry_on_overload(...)`.

## Best practices

- Always call `initialize()` once after client start.
- Use context managers (`with AppServerClient() as client`) to guarantee cleanup.
- Keep a single client per process/thread where possible.
- Treat notification streams as ordered and consume continuously during active turns.
- Gate real integration tests with `RUN_REAL_CODEX_TESTS=1` in CI environments that have `codex` configured.

## Tests

```bash
cd sdk/python
pytest
```

Optional real integration tests:

```bash
RUN_REAL_CODEX_TESTS=1 pytest tests/test_real_app_server_integration.py
```

## Related docs

- `CHANGELOG.md` — milestone history for this SDK
- `CONTRIBUTING.md` — development workflow and quality gates
- `RELEASE_CHECKLIST.md` — release gate checklist
- `learning.md` — SDK patterns extracted from top Python SDKs
- `APP_SERVER_V2_NOTES.md` — architecture + method map for Codex app-server v2
