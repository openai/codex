# Getting Started

This is the fastest path from install to a multi-turn thread using the public SDK surface.

The SDK is experimental. Treat the API, bundled runtime strategy, and packaging details as unstable until the first public release.

## 1) Install

From repo root:

```bash
cd sdk/python
python -m pip install -e .
```

Requirements:

- Python `>=3.10`
- installed `codex-cli-bin` runtime package, or an explicit `codex_bin` override
- local Codex auth/session configured

## 2) Run your first turn (sync)

```python
from codex_app_server import Codex, TextInput

with Codex() as codex:
    server = codex.metadata.serverInfo
    print("Server:", None if server is None else server.name, None if server is None else server.version)

    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
    completed_turn = thread.turn(TextInput("Say hello in one sentence.")).run()

    print("Thread:", thread.id)
    print("Turn:", completed_turn.id)
    print("Status:", completed_turn.status)
    print("Items:", len(completed_turn.items or []))
```

What happened:

- `Codex()` started and initialized `codex app-server`.
- `thread_start(...)` created a thread.
- `turn(...).run()` consumed events until `turn/completed` and returned the canonical generated app-server `Turn` model.
- one client can have only one active `TurnHandle.stream()` / `TurnHandle.run()` consumer at a time in the current experimental build

## 3) Continue the same thread (multi-turn)

```python
from codex_app_server import Codex, TextInput

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})

    first = thread.turn(TextInput("Summarize Rust ownership in 2 bullets.")).run()
    second = thread.turn(TextInput("Now explain it to a Python developer.")).run()

    print("first:", first.id, first.status)
    print("second:", second.id, second.status)
```

## 4) Async parity

```python
import asyncio
from codex_app_server import AsyncCodex, TextInput


async def main() -> None:
    async with AsyncCodex() as codex:
        thread = await codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
        turn = await thread.turn(TextInput("Continue where we left off."))
        completed_turn = await turn.run()
        print(completed_turn.id, completed_turn.status)


asyncio.run(main())
```

## 5) Resume an existing thread

```python
from codex_app_server import Codex, TextInput

THREAD_ID = "thr_123"  # replace with a real id

with Codex() as codex:
    thread = codex.thread_resume(THREAD_ID)
    completed_turn = thread.turn(TextInput("Continue where we left off.")).run()
    print(completed_turn.id, completed_turn.status)
```

## 6) Generated models

The convenience wrappers live at the package root, but the canonical app-server models live under:

```python
from codex_app_server.generated.v2_all import Turn, TurnStatus, ThreadReadResponse
```

## 7) Next stops

- API surface and signatures: `docs/api-reference.md`
- Common decisions/pitfalls: `docs/faq.md`
- End-to-end runnable examples: `examples/README.md`
