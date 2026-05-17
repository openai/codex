# Getting Started

This is the fastest path from install to a multi-turn thread using the public SDK surface.

The SDK is experimental, so the public API and runtime requirements may keep evolving before the first public release.

## 1) Install

From repo root:

```bash
cd sdk/python
uv sync
source .venv/bin/activate
```

Requirements:

- Python `>=3.10`
- uv
- installed `openai-codex-cli-bin` runtime package, or an explicit `codex_bin` override

## 2) Authenticate when needed

Existing Codex auth state is reused automatically. To authenticate from the SDK,
use the flow that fits your app:

```python
from openai_codex import Codex

with Codex() as codex:
    codex.login_api_key("sk-...")
    account = codex.account()
    print(account.account)
```

Interactive ChatGPT browser login returns a handle that carries the URL and the
matching completion event:

```python
with Codex() as codex:
    login = codex.login_chatgpt()
    print(login.auth_url)
    completed = login.wait()
    print(completed.success)
```

Device-code login works the same way with
`login_chatgpt_device_code()`, which exposes `verification_url`, `user_code`,
and `wait()`.

## 3) Run your first turn (sync)

```python
from openai_codex import Codex

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
- `result.final_response` is `None` when no final-answer or phase-less assistant message item completes for the turn.
- use `thread.turn(...)` when you need a `TurnHandle` for streaming, steering, interrupting, or turn IDs/status
- one client can consume multiple active turns concurrently; turn streams are routed by turn ID

## 4) Continue the same thread (multi-turn)

```python
from openai_codex import Codex

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})

    first = thread.run("Summarize Rust ownership in 2 bullets.")
    second = thread.run("Now explain it to a Python developer.")

    print("first:", first.final_response)
    print("second:", second.final_response)
```

## 5) Async parity

Use `async with AsyncCodex()` as the normal async entrypoint. `AsyncCodex`
initializes lazily, and context entry makes startup/shutdown explicit.

```python
import asyncio
from openai_codex import AsyncCodex


async def main() -> None:
    async with AsyncCodex() as codex:
        thread = await codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
        result = await thread.run("Continue where we left off.")
        print(result.final_response)


asyncio.run(main())
```

## 6) Resume an existing thread

```python
from openai_codex import Codex

THREAD_ID = "thr_123"  # replace with a real id

with Codex() as codex:
    thread = codex.thread_resume(THREAD_ID)
    result = thread.run("Continue where we left off.")
    print(result.final_response)
```

## 7) Public app-server types

The convenience wrappers live at the package root. Public app-server value and
event types live under:

```python
from openai_codex.types import ThreadReadResponse, Turn, TurnStatus
```

## 8) Next stops

- API surface and signatures: `docs/api-reference.md`
- Common decisions/pitfalls: `docs/faq.md`
- End-to-end runnable examples: `examples/README.md`
