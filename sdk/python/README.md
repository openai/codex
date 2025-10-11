# Codex Python SDK

Embed the Codex agent in Python workflows. This SDK shells out to the bundled `codex` CLI, streams
structured events, and provides strongly-typed helpers for synchronous and streaming turns.

## Status

- Target Python 3.12+.
- API and packaging are pre-alpha; expect breaking changes.
- Binaries are bundled under `codex/vendor` for supported triples.

## Quickstart

```python
from codex import Codex

client = Codex()
thread = client.start_thread()
turn = thread.run("Summarize the latest CI failure.")

print(turn.final_response)
for item in turn.items:
    print(item)
```

## Streaming

```python
from codex import Codex

client = Codex()
thread = client.start_thread()

stream = thread.run_streamed("Implement the fix.")
for event in stream:
    print(event)
```

## Structured Output

```python
from codex import Codex, TurnOptions

schema = {
    "type": "object",
    "properties": {
        "summary": {"type": "string"},
        "status": {"type": "string", "enum": ["ok", "action_required"]},
    },
    "required": ["summary", "status"],
    "additionalProperties": False,
}

thread = Codex().start_thread()
turn = thread.run("Summarize repository status", TurnOptions(output_schema=schema))
print(turn.final_response)
```

### Structured output with Pydantic (optional)

If you use [Pydantic](https://docs.pydantic.dev/latest/) v2, you can pass a model class or instance directly. The SDK converts it to JSON Schema automatically:

```python
from pydantic import BaseModel
from codex import Codex, TurnOptions


class StatusReport(BaseModel):
    summary: str
    status: str


thread = Codex().start_thread()
turn = thread.run(
    "Summarize repository status",
    TurnOptions(output_schema=StatusReport),
)
print(turn.final_response)
```

## Development

- Install dependencies with `uv sync --extra dev`.
- Run formatting and linting: `uv run ruff check .` and `uv run ruff format .`.
- Type-check with `uv run mypy --config-file pyproject.toml src/codex`.
- Tests via `uv run pytest`.

### Bundling native binaries

The SDK shells out to the Rust `codex` executable. For local testing we point at
`codex-rs/target/debug/codex`, but release builds should bundle the official
artifacts in `src/codex/vendor/` just like the TypeScript SDK. Use the helper
script to fetch prebuilt binaries from the Rust release workflow:

```bash
uv run python sdk/python/scripts/install_native_deps.py --clean --workflow-url <workflow-url>
```

Omit `--workflow-url` to use the default pinned run. After bundling, build the
wheel/sdist with `uv build` (or `python -m build`). The `vendor/` directory is
ignored by git aside from its README, so remember to run the script before
cutting a release.

