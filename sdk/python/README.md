# Codex App Server Python SDK (Experimental)

Experimental Python SDK for `codex app-server` JSON-RPC v2 over stdio, with a small default surface optimized for real scripts and apps.

The generated wire-model layer is currently sourced from the bundled v2 schema and exposed as Pydantic models with snake_case Python fields that serialize back to the app-server’s camelCase wire format.

## Install

```bash
cd sdk/python
python -m pip install -e .
```

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

## Bundled runtime binaries (out of the box)

The SDK ships with platform-specific bundled binaries, so end users do not need updater scripts.

Runtime binary source (single source, no fallback):

- `src/codex_app_server/bin/darwin-arm64/codex`
- `src/codex_app_server/bin/darwin-x64/codex`
- `src/codex_app_server/bin/linux-arm64/codex`
- `src/codex_app_server/bin/linux-x64/codex`
- `src/codex_app_server/bin/windows-arm64/codex.exe`
- `src/codex_app_server/bin/windows-x64/codex.exe`

## Maintainer workflow (refresh binaries/types)

```bash
cd sdk/python
python scripts/update_sdk_artifacts.py --channel stable --bundle-all-platforms
# or
python scripts/update_sdk_artifacts.py --channel alpha --bundle-all-platforms
```

This refreshes all bundled OS/arch binaries and regenerates protocol-derived Python types.

## Compatibility and versioning

- Package: `codex-app-server-sdk`
- Current SDK version in this repo: `0.2.0`
- Python: `>=3.10`
- Target protocol: Codex `app-server` JSON-RPC v2
- Recommendation: keep SDK and `codex` CLI reasonably up to date together

## Notes

- `Codex()` is eager and performs startup + `initialize` in the constructor.
- Use context managers (`with Codex() as codex:`) to ensure shutdown.
- For transient overload, use `codex_app_server.retry.retry_on_overload`.
