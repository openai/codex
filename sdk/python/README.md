# Codex App Server Python SDK (v2)

Python SDK for `codex app-server` JSON-RPC v2 over stdio.

## Status

- ✅ initialize + initialized handshake
- ✅ core thread/turn methods
- ✅ streaming notification consumption
- ✅ command/file approval request handling
- ✅ integration-test harness with fake app-server
- ✅ async client (`AsyncAppServerClient`)
- ✅ fluent thread abstraction (`Conversation` / `AsyncConversation`)
- ✅ typed convenience wrappers (`ThreadStartResult`, `TurnStartResult`)
- ✅ schema-backed typed response helpers (`thread_start_schema`, `turn_start_schema`, ...)
- ✅ protocol TypedDicts for v2 core responses (`ThreadStartResponse`, `TurnStartResponse`, etc.)
- ✅ optional real app-server integration test (gated by env var)
- 🔜 full generated models from app-server JSON schema

## Install (editable)

```bash
cd sdk/python
python -m pip install -e .
```

## Quickstart

```python
from codex_app_server import AppServerClient

with AppServerClient() as client:
    client.initialize()

    thread = client.thread_start(model="gpt-5")
    thread_id = thread["thread"]["id"]

    # ergonomic text-only turn (equivalent to input=[{"type": "text", ...}])
    turn = client.turn_text(thread_id, "Explain Newton's method in 3 bullets")
    turn_id = turn["turn"]["id"]

    # stream until this turn finishes
    final = client.wait_for_turn_completed(turn_id)
    print(final.method, final.params["turn"]["status"])
```

## Notebook-style usage

```python
from codex_app_server import AppServerClient

client = AppServerClient()
client.start()
client.initialize()

thread_id = client.thread_start(model="gpt-5")["thread"]["id"]

# turn_start also accepts raw text directly
turn = client.turn_start(thread_id, "summarize this repo architecture")

events = client.stream_until_methods("turn/completed")
for e in events:
    if e.method == "item/agentMessage/delta":
        print((e.params or {}).get("delta", ""), end="")
```

## Cookbook

### Fluent conversation API

```python
from codex_app_server import AppServerClient

with AppServerClient() as client:
    client.initialize()
    conv = client.conversation_start(model="gpt-5")

    # one-liner ask on the same thread
    answer = conv.ask("Summarize the last release notes")
    print(answer)

    # explicit turn start helpers still available
    turn = conv.turn_text("Give 3 follow-up tasks")
    print(turn["turn"]["id"])
```

### Stream notifications for a single turn

```python
with AppServerClient() as client:
    client.initialize()
    conv = client.conversation_start(model="gpt-5")

    for evt in conv.stream("Explain this stacktrace"):
        if evt.method == "item/agentMessage/delta":
            print((evt.params or {}).get("delta", ""), end="")
```

### Strongly typed schema responses (without changing dict API)

```python
with AppServerClient() as client:
    client.initialize()

    started = client.thread_start_schema(model="gpt-5")
    print(started.thread.id)  # dataclass field

    turn = client.turn_text_schema(started.thread.id, "hello")
    print(turn.turn.status)
```

## Async quickstart

```python
import asyncio
from codex_app_server import AsyncAppServerClient

async def main():
    async with AsyncAppServerClient() as client:
        await client.initialize()
        thread = await client.thread_start(model="gpt-5")
        turn = await client.turn_text(thread["thread"]["id"], "hello from async")
        await client.wait_for_turn_completed(turn["turn"]["id"])

asyncio.run(main())
```

## API surface (v0.1)

- `initialize()`
- `thread_start(**params)`
- `thread_resume(thread_id, **params)`
- `thread_list(**params)`
- `thread_read(thread_id, include_turns=False)`
- `turn_start(thread_id, input_items, **params)` (`input_items` can be list/dict/str)
- `turn_text(thread_id, text, **params)`
- `turn_interrupt(thread_id, turn_id)`
- `model_list(include_hidden=False)`
- `next_notification()`
- `wait_for_turn_completed(turn_id)`
- `ask(text, model=None, thread_id=None)` notebook helper
- conversation helpers: `conversation(thread_id)`, `conversation_start(model=..., **params)`
- typed wrappers: `thread_start_typed()`, `turn_start_typed()`
- schema-backed typed helpers: `thread_start_schema()`, `thread_list_schema()`, `turn_start_schema()`, `turn_text_schema()`

## Design goals

- Keep API ergonomic for notebooks and scripts
- Keep protocol surface close to app-server v2 names
- Offer low-level JSON-RPC escape hatch (`request`, `notify`)
- Keep approval flow pluggable via `approval_handler`

## Tests

```bash
cd sdk/python
pytest
```

Run optional real integration tests (requires `codex` binary in PATH and auth/config set):

```bash
RUN_REAL_CODEX_TESTS=1 pytest tests/test_real_app_server_integration.py
```

## Related docs

- `learning.md` — SDK patterns extracted from top Python SDKs
- `APP_SERVER_V2_NOTES.md` — architecture + method map for Codex app-server v2
