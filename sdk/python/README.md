# Codex App Server Python SDK (Experimental)

Experimental Python SDK for `codex app-server` JSON-RPC v2 over stdio, with a small default surface optimized for real scripts and apps.

The generated wire-model layer is currently sourced from the bundled v2 schema and exposed as Pydantic models with snake_case Python fields that serialize back to the app-server’s camelCase wire format.

## Install

```bash
cd sdk/python
python -m pip install -e .
```

Published SDK builds pin an exact `codex-cli-bin` runtime dependency. For local
repo development, make `codex` available on `PATH` or pass
`AppServerConfig(codex_bin=...)` to point at a local build explicitly.

## Quickstart

```python
from codex_app_server import Codex, TextInput

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5")
    result = thread.turn(TextInput("Say hello in one sentence.")).run()
    print(result.text)
```

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

## Runtime packaging

The repo no longer checks `codex` binaries into `sdk/python`.

Published SDK builds are pinned to an exact `codex-cli-bin` package version,
and that runtime package carries the platform-specific binary for the target
wheel.

For local repo development, the checked-in `sdk/python-runtime` package is only
a template for staged release artifacts. Editable installs should use a local
`codex` on `PATH` or an explicit `codex_bin` override instead.

## Maintainer workflow (refresh binaries/types)

```bash
cd sdk/python
python scripts/update_sdk_artifacts.py --types-only
python scripts/update_sdk_artifacts.py \
  --stage-release \
  --output-dir /tmp/codex-python-release \
  --channel stable
```

This regenerates protocol-derived Python types, then stages:

- `codex-app-server-sdk` with an exact `codex-cli-bin==...` dependency
- `codex-cli-bin` for the current platform with the matching `codex` binary

## Compatibility and versioning

- Package: `codex-app-server-sdk`
- Runtime package: `codex-cli-bin`
- Current SDK version in this repo: `0.2.0`
- Python: `>=3.10`
- Target protocol: Codex `app-server` JSON-RPC v2
- Recommendation: keep SDK and `codex` CLI reasonably up to date together

## Notes

- `Codex()` is eager and performs startup + `initialize` in the constructor.
- Use context managers (`with Codex() as codex:`) to ensure shutdown.
- For transient overload, use `codex_app_server.retry.retry_on_overload`.
