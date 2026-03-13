# Codex App Server Python SDK (Experimental)

Experimental Python SDK for `codex app-server` JSON-RPC v2 over stdio, with a small default surface optimized for real scripts and apps.

The generated wire-model layer is currently sourced from the bundled v2 schema and exposed as Pydantic models with snake_case Python fields that serialize back to the app-server’s camelCase wire format.

## Install

```bash
cd sdk/python
python -m pip install -e .
```

This checked-in package is the runtime-free core distribution:
`codex-app-server-sdk-core`.

Published releases expose two Python package names:

- `codex-app-server-sdk-core`: the actual Python SDK code, without bundled binaries
- `codex-app-server-sdk`: a bundled metapackage that depends on `codex-app-server-sdk-core` and `codex-cli-bin`

For local repo development, pass `AppServerConfig(codex_bin=...)` to point at
a local build explicitly.

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

Published `codex-app-server-sdk` builds depend on an exact
`codex-app-server-sdk-core` version plus an exact `codex-cli-bin` version, and
that runtime package carries the platform-specific binary for the target wheel.

For local repo development, the checked-in `sdk/python-runtime` package is only
a template for staged release artifacts. Editable installs should use an
explicit `codex_bin` override instead.

## Maintainer workflow

```bash
cd sdk/python
python scripts/update_sdk_artifacts.py generate-types
python scripts/update_sdk_artifacts.py \
  stage-sdk-core \
  /tmp/codex-python-release/codex-app-server-sdk-core \
  --skip-generate-types \
  --sdk-version 1.2.3
python scripts/update_sdk_artifacts.py \
  stage-sdk \
  /tmp/codex-python-release/codex-app-server-sdk \
  --skip-generate-types \
  --sdk-version 1.2.3 \
  --runtime-version 1.2.3
python scripts/update_sdk_artifacts.py \
  stage-runtime \
  /tmp/codex-python-release/codex-cli-bin \
  /path/to/codex \
  --runtime-version 1.2.3
```

This supports the CI release flow:

- run `generate-types` before packaging
- stage `codex-app-server-sdk-core` with the release tag version as `--sdk-version`
- stage `codex-app-server-sdk` as a bundled metapackage pinned to exact `codex-app-server-sdk-core==...` and `codex-cli-bin==...`
- stage `codex-cli-bin` on each supported platform runner with the same pinned runtime version
- build and publish `codex-cli-bin` as platform wheels only; do not publish an sdist
- publish `codex-app-server-sdk-core` to PyPI after the runtime wheels land
- publish `codex-app-server-sdk` to PyPI last, using the same release version

## Compatibility and versioning

- Core package: `codex-app-server-sdk-core`
- Bundled package: `codex-app-server-sdk`
- Runtime package: `codex-cli-bin`
- Current SDK version in this repo: `0.2.0`
- Python: `>=3.10`
- Target protocol: Codex `app-server` JSON-RPC v2
- Release policy: published Python package versions should match the `rust-v...` release tag
- Recommendation: keep SDK and `codex` CLI reasonably up to date together

## Notes

- `Codex()` is eager and performs startup + `initialize` in the constructor.
- Use context managers (`with Codex() as codex:`) to ensure shutdown.
- For transient overload, use `codex_app_server.retry.retry_on_overload`.
