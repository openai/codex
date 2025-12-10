# Codex SDK for Python

Embed the Codex agent in Python workflows by spawning the Codex CLI and consuming structured events.

The SDK launches the bundled `codex` binary (or a custom path) and exchanges JSONL events over stdin/stdout.

## Installation

Until packages are published, install locally in editable mode:

```
pip install -e .
```

Requires Python 3.9+ and a Codex binary reachable either via the packaged vendor path or `codex_path_override`.

## Quickstart

```python
from codex_sdk import Codex

codex = Codex()
thread = codex.start_thread()
turn = thread.run("Diagnose the test failure and propose a fix")

print(turn.final_response)
print(turn.items)
```

Call `run()` again on the same `Thread` to continue the conversation.

```python
next_turn = thread.run("Implement the fix")
```

## Streaming responses

`run()` buffers events. To react to intermediate progress—tool calls, streamed responses, and file change notifications—use `run_streamed()` instead. It returns a generator of structured events.

```python
from codex_sdk import ThreadEvent

stream = thread.run_streamed("Diagnose the test failure and propose a fix")

for event in stream.events:
    if event.type == "item.completed":
        print("item", event.item)
    elif event.type == "turn.completed":
        print("usage", event.usage)
```

## Structured output

Provide a JSON schema per turn to receive structured assistant responses.

```python
schema = {
    "type": "object",
    "properties": {
        "summary": {"type": "string"},
        "status": {"type": "string", "enum": ["ok", "action_required"]},
    },
    "required": ["summary", "status"],
    "additionalProperties": False,
}

from codex_sdk import TurnOptions

turn = thread.run("Summarize repository status", TurnOptions(output_schema=schema))
print(turn.final_response)
```

## Attaching images

Pass structured input entries when including images alongside text. Text entries are concatenated into the prompt; image entries are forwarded to the Codex CLI via `--image`.

```python
turn = thread.run([
    {"type": "text", "text": "Describe these screenshots"},
    {"type": "local_image", "path": "./ui.png"},
    {"type": "local_image", "path": "./diagram.jpg"},
])
```

## Resuming an existing thread

Threads persist in `~/.codex/sessions`. If you lose the in-memory `Thread`, reconstruct it with `resume_thread()` and keep going.

```python
import os

saved_thread_id = os.environ["CODEX_THREAD_ID"]
thread = codex.resume_thread(saved_thread_id)
thread.run("Implement the fix")
```

## Working directory controls

Codex runs in the current working directory by default. To bypass the Git repository check for temporary directories, pass `skip_git_repo_check=True` when creating a thread.

```python
from codex_sdk import ThreadOptions

thread = codex.start_thread(ThreadOptions(working_directory="/path/to/project", skip_git_repo_check=True))
```

## Controlling the Codex CLI environment

By default, the CLI inherits `os.environ`. Override it when you need a sandboxed environment and the SDK will inject required variables (`OPENAI_BASE_URL`, `CODEX_API_KEY`, and the SDK originator marker).

```python
from codex_sdk import CodexOptions

codex = Codex(CodexOptions(env={"PATH": "/usr/local/bin"}))
```

## Options reference

- `CodexOptions`: `codex_path_override`, `base_url`, `api_key`, `env`
- `ThreadOptions`: `model`, `sandbox_mode`, `working_directory`, `skip_git_repo_check`, `model_reasoning_effort`, `network_access_enabled`, `web_search_enabled`, `approval_policy`, `additional_directories`
- `TurnOptions`: `output_schema`, `cancellation_event`

## Running tests

```
python -m unittest discover -v
```

Set `codex_path_override` if the bundled binary is unavailable; the test suite expects a Codex binary and will exercise a local HTTP proxy to avoid external calls.
