# Codex App Server Python SDK (v2)

Python SDK for `codex app-server` JSON-RPC v2 over stdio.

## Status

- ✅ initialize + initialized handshake
- ✅ core thread/turn methods
- ✅ streaming notification consumption
- ✅ command/file approval request handling
- ✅ integration-test harness with fake app-server
- ✅ async client (`AsyncAppServerClient`)
- ✅ typed convenience wrappers (`ThreadStartResult`, `TurnStartResult`)
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

    turn = client.turn_start(
        thread_id,
        input_items=[{"type": "text", "text": "Explain Newton's method in 3 bullets"}],
    )
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

turn = client.turn_start(
    thread_id,
    input_items=[{"type": "text", "text": "summarize this repo architecture"}],
)

events = client.stream_until_methods({"turn/completed"})
for e in events:
    if e.method == "item/agentMessage/delta":
        print((e.params or {}).get("delta", ""), end="")
```

## Async quickstart

```python
import asyncio
from codex_app_server import AsyncAppServerClient

async def main():
    async with AsyncAppServerClient() as client:
        await client.initialize()
        thread = await client.thread_start(model="gpt-5")
        turn = await client.turn_start(
            thread["thread"]["id"],
            [{"type": "text", "text": "hello from async"}],
        )
        await client.wait_for_turn_completed(turn["turn"]["id"])

asyncio.run(main())
```

## API surface (v0.1)

- `initialize()`
- `thread_start(**params)`
- `thread_resume(thread_id, **params)`
- `thread_list(**params)`
- `thread_read(thread_id, include_turns=False)`
- `turn_start(thread_id, input_items, **params)`
- `turn_interrupt(thread_id, turn_id)`
- `model_list(include_hidden=False)`
- `next_notification()`
- `wait_for_turn_completed(turn_id)`
- `ask(text, model=None, thread_id=None)` notebook helper
- typed wrappers: `thread_start_typed()`, `turn_start_typed()`

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
