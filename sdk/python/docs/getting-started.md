# Getting Started

This is the fastest path from install to a multi-turn thread using the minimal SDK surface.

## 1) Install

From repo root:

```bash
cd sdk/python
python -m pip install -e .
```

Requirements:

- Python `>=3.10`
- installed `codex-cli-bin` runtime package, or an explicit `codex_bin` override
- Local Codex auth/session configured

## 2) Run your first turn

```python
from codex_app_server import Codex, TextInput

with Codex() as codex:
    print("Server:", codex.metadata.server_name, codex.metadata.server_version)

    thread = codex.thread_start(model="gpt-5")
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

## 3) Continue the same thread (multi-turn)

```python
from codex_app_server import Codex, TextInput

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5")

    first = thread.turn(TextInput("Summarize Rust ownership in 2 bullets.")).run()
    second = thread.turn(TextInput("Now explain it to a Python developer.")).run()

    print("first:", first.text)
    print("second:", second.text)
```

## 4) Resume an existing thread

```python
from codex_app_server import Codex, TextInput

THREAD_ID = "thr_123"  # replace with a real id

with Codex() as codex:
    thread = codex.thread(THREAD_ID)
    result = thread.turn(TextInput("Continue where we left off.")).run()
    print(result.text)
```

## 5) Next stops

- API surface and signatures: `docs/api-reference.md`
- Common decisions/pitfalls: `docs/faq.md`
- End-to-end runnable examples: `examples/README.md`
