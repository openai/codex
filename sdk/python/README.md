# Codex Python SDK

Python wrapper for the Codex CLI. Provides sync and asyncio APIs that mirror the TypeScript SDK.

## Requirements
- Python 3.11+
- Codex CLI installed (`npm install -g @openai/codex` or `brew install --cask codex`)
- Signed in with ChatGPT (Plus/Pro/Team/Edu/Enterprise) or an API key

## Quickstart (sync)
```python
from codex_sdk import Codex

codex = Codex()
thread = codex.start_thread()
turn = thread.run("Diagnose the test failure and propose a fix")

print(turn.final_response)
print(turn.items)
```

## Asyncio
```python
import asyncio
from codex_sdk.asyncio import AsyncCodex

async def main() -> None:
    codex = AsyncCodex()
    thread = codex.start_thread()
    turn = await thread.run("Implement the fix")
    print(turn.final_response)

asyncio.run(main())
```

## Streaming
```python
from codex_sdk import Codex

codex = Codex()
thread = codex.start_thread()
streamed = thread.run_streamed("Diagnose the test failure")

for event in streamed.events:
    if event["type"] == "item.completed":
        print(event["item"])
```

## Structured output (Pydantic)
```python
from pydantic import BaseModel
from codex_sdk import Codex

class SummarySchema(BaseModel):
    summary: str
    status: str

codex = Codex()
thread = codex.start_thread()
turn = thread.run("Summarize repository status", output_schema=SummarySchema)
print(turn.final_response)
```

## Images
```python
from codex_sdk import Codex

codex = Codex()
thread = codex.start_thread()
turn = thread.run(
    [
        {"type": "text", "text": "Describe these screenshots"},
        {"type": "local_image", "path": "./ui.png"},
        {"type": "local_image", "path": "./diagram.jpg"},
    ]
)
```
