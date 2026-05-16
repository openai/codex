# OpenAI Codex Python SDK (Experimental)

Experimental Python SDK for `codex app-server` JSON-RPC v2 over stdio, with a small default surface optimized for real scripts and apps.

The generated wire-model layer is sourced from the pinned `openai-codex-cli-bin`
runtime package and exposed as Pydantic models with snake_case Python fields
that serialize back to the app-server’s camelCase wire format.
The package root exports the ergonomic client API; public app-server value and
event types live in `openai_codex.types`.

## Install

```bash
cd sdk/python
uv sync
source .venv/bin/activate
```

Published SDK builds pin an exact `openai-codex-cli-bin` runtime dependency
with the same version as the SDK. Pass `AppServerConfig(codex_bin=...)` only
when you intentionally want to run against a specific local app-server binary.

## Quickstart

```python
from openai_codex import Codex

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5")
    result = thread.run("Say hello in one sentence.")
    print(result.final_response)
    print(len(result.items))
```

`result.final_response` is `None` when the turn completes without a final-answer
or phase-less assistant message item.

## Docs map

- Golden path tutorial: `docs/getting-started.md`
- API reference (signatures + behavior): `docs/api-reference.md`
- Common decisions and pitfalls: `docs/faq.md`
- Runnable examples index: `examples/README.md`
- Jupyter walkthrough notebook: `notebooks/sdk_walkthrough.ipynb`

## Examples

Start here:

```bash
cd sdk/python
python examples/01_quickstart_constructor/sync.py
python examples/01_quickstart_constructor/async.py
```

## Runtime

Published SDK builds are pinned to an exact `openai-codex-cli-bin` package
version, and that runtime package carries the platform-specific binary for the
target wheel. The SDK package version and runtime package version must match.

## Compatibility and versioning

- Package: `openai-codex`
- Runtime package: `openai-codex-cli-bin`
- Python: `>=3.10`
- Target protocol: Codex `app-server` JSON-RPC v2
- Versioning rule: the SDK package version is the underlying Codex runtime version

## Notes

- `Codex()` is eager and performs startup + `initialize` in the constructor.
- Use context managers (`with Codex() as codex:`) to ensure shutdown.
- Prefer `thread.run("...")` for the common case. Use `thread.turn(...)` when
  you need streaming, steering, or interrupt control.
- For transient overload, use `retry_on_overload` from the package root.
