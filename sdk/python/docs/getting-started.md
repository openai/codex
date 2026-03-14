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
    print("Server:", codex.metadata.server_name, codex.metadata.server_version)

    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
    result = thread.turn(TextInput("Say hello in one sentence.")).run()

    print("Thread:", result.thread_id)
    print("Turn:", result.turn_id)
    print("Status:", result.status)
    print("Text:", result.text)
```

What happened:

- `Codex()` started and initialized `codex app-server`.
- `thread_start(...)` created a thread.
- `turn(...).run()` consumed events until `turn/completed` and returned a `TurnResult`.
- one client can have only one active `Turn.stream()` / `Turn.run()` consumer at a time in the current experimental build

## 3) Continue the same thread (multi-turn)

```python
from codex_app_server import Codex, TextInput

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})

    first = thread.turn(TextInput("Summarize Rust ownership in 2 bullets.")).run()
    second = thread.turn(TextInput("Now explain it to a Python developer.")).run()

    print("first:", first.text)
    print("second:", second.text)
```

## 4) Async parity

```python
import asyncio
from codex_app_server import AsyncCodex, TextInput


async def main() -> None:
    async with AsyncCodex() as codex:
        thread = await codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
        turn = await thread.turn(TextInput("Continue where we left off."))
        result = await turn.run()
        print(result.text)


asyncio.run(main())
```

## 5) Resume an existing thread

```python
from codex_app_server import Codex, TextInput

THREAD_ID = "thr_123"  # replace with a real id

with Codex() as codex:
    thread = codex.thread_resume(THREAD_ID)
    result = thread.turn(TextInput("Continue where we left off.")).run()
    print(result.text)
```

## 6) Next stops

- API surface and signatures: `docs/api-reference.md`
- Common decisions/pitfalls: `docs/faq.md`
- End-to-end runnable examples: `examples/README.md`
