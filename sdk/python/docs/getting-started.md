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
from codex_app_server import Codex

with Codex() as codex:
    server = codex.metadata.serverInfo
    print("Server:", None if server is None else server.name, None if server is None else server.version)

    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
    result = thread.run("Say hello in one sentence.")

    print("Thread:", thread.id)
    print("Text:", result.final_response)
    print("Items:", len(result.items))
```

What happened:

- `Codex()` started and initialized `codex app-server`.
- `thread_start(...)` created a thread.
- `thread.run("...")` started a turn, consumed events until completion, and returned the final assistant response plus collected items and usage.
- use `thread.turn(...)` when you need a `TurnHandle` for streaming, steering, interrupting, or turn IDs/status
- one client can have only one active turn consumer (`thread.run(...)`, `TurnHandle.stream()`, or `TurnHandle.run()`) at a time in the current experimental build

## 3) Continue the same thread (multi-turn)

```python
from codex_app_server import Codex

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})

    first = thread.run("Summarize Rust ownership in 2 bullets.")
    second = thread.run("Now explain it to a Python developer.")

    print("first:", first.final_response)
    print("second:", second.final_response)
```

## 4) Async parity

Use `async with AsyncCodex()` as the normal async entrypoint. `AsyncCodex`
initializes lazily, and context entry makes startup/shutdown explicit.

```python
import asyncio
from codex_app_server import AsyncCodex


async def main() -> None:
    async with AsyncCodex() as codex:
        thread = await codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
        result = await thread.run("Continue where we left off.")
        print(result.final_response)


asyncio.run(main())
```

## 5) Resume an existing thread

```python
from codex_app_server import Codex

THREAD_ID = "thr_123"  # replace with a real id

with Codex() as codex:
    thread = codex.thread_resume(THREAD_ID)
    result = thread.run("Continue where we left off.")
    print(result.final_response)
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
