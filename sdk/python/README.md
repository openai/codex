# Codex App Server Python SDK (Experimental)

Experimental Python SDK for `codex app-server` JSON-RPC v2.
The generated wire models come from the bundled v2 schema and use snake_case Python fields while preserving camelCase wire serialization.

It gives you a small typed API for:

- starting or resuming threads
- creating turns from Python
- streaming events or waiting for a final `TurnResult`
- using the same shape in sync and async code

## Experimental

This SDK is still experimental.

- it is not published yet
- API details may still change before the first release
- packaging and release workflow are still evolving

Use it for local development, dogfooding, and iteration inside this repo. Do not treat it as a stable public package yet.

## What You Need

- Python `>=3.10`
- local Codex auth/session already configured
- this repo checked out locally

## Install From Source

```bash
cd sdk/python
python -m pip install -e .
```

The package includes bundled Codex runtime binaries and automatically selects the binary for the current platform through `AppServerConfig().codex_bin`.

## Core Model

The public API is intentionally small:

- `Codex` / `AsyncCodex`: session entrypoint
- `Thread` / `AsyncThread`: a conversation thread
- `Turn` / `AsyncTurn`: one user turn within a thread
- `TurnResult`: final status, text, items, and usage

Typical flow:

1. create a `Codex` client
2. start or resume a thread
3. create a turn from input
4. call `run()` or iterate `stream()`

## Quickstart

### Sync

```python
from codex_app_server import Codex, TextInput

with Codex() as codex:
    thread = codex.thread_start(
        model="gpt-5",
        config={"model_reasoning_effort": "high"},
    )
    result = thread.turn(TextInput("Say hello in one sentence.")).run()

    print("status:", result.status)
    print("text:", result.text)
```

### Async

```python
import asyncio

from codex_app_server import AsyncCodex, TextInput


async def main() -> None:
    async with AsyncCodex() as codex:
        thread = await codex.thread_start(
            model="gpt-5",
            config={"model_reasoning_effort": "high"},
        )
        turn = await thread.turn(TextInput("Say hello in one sentence."))
        result = await turn.run()

        print("status:", result.status)
        print("text:", result.text)


asyncio.run(main())
```

## Current Limitations

- Only one active `Turn.stream()` or `Turn.run()` consumer is supported per client instance.
- Starting a second active turn consumer on the same `Codex` or `AsyncCodex` raises `RuntimeError`.
- `Codex()` is eager and performs startup plus `initialize` in the constructor.

## Behavior Notes

- `AsyncCodex` is intended to be used with `async with AsyncCodex() as codex:`.
- `TurnResult.text` prefers streamed assistant deltas and falls back to completed raw response items when no deltas are emitted.
- For transient overload handling, use `retry_on_overload(...)`.

## Learn By Example

Runnable examples:

```bash
cd sdk/python
python examples/01_quickstart_constructor/sync.py
python examples/01_quickstart_constructor/async.py
```

More docs:

- Getting started: `docs/getting-started.md`
- API reference: `docs/api-reference.md`
- FAQ and pitfalls: `docs/faq.md`
- Examples index: `examples/README.md`
- Notebook walkthrough: `notebooks/sdk_walkthrough.ipynb`

## Maintainer Workflow

Refresh bundled binaries and generated artifacts with:

```bash
cd sdk/python
python scripts/update_sdk_artifacts.py --channel stable --bundle-all-platforms
```

or:

```bash
cd sdk/python
python scripts/update_sdk_artifacts.py --channel alpha --bundle-all-platforms
```

## Compatibility

- Package name: `codex-app-server-sdk`
- SDK version in this repo: `0.2.0`
- Target protocol: Codex `app-server` JSON-RPC v2
