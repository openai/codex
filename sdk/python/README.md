# Codex App Server Python SDK (Experimental)

Experimental Python SDK for `codex app-server` JSON-RPC v2 over stdio, with a small default surface optimized for real scripts and apps.

The generated wire-model layer is currently sourced from the bundled v2 schema and exposed as Pydantic models with snake_case Python fields that serialize back to the app-server’s camelCase wire format.

## Install

```bash
cd sdk/python
uv sync
source .venv/bin/activate
```

Published SDK builds pin an exact `openai-codex-cli-bin` runtime dependency
with the same version as the SDK. For local repo development, either pass
`AppServerConfig(codex_bin=...)` to point at a local build explicitly, or use
the repo examples/notebook bootstrap which installs the pinned runtime package
automatically.

## Quickstart

```python
from codex_app_server import Codex

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

## Runtime packaging

The repo no longer checks `codex` binaries into `sdk/python`.

Published SDK builds are pinned to an exact `openai-codex-cli-bin` package
version, and that runtime package carries the platform-specific binary for the
target wheel. The SDK package version and runtime package version must match.

For local repo development, the checked-in `sdk/python-runtime` package is only
a template for staged release artifacts. Editable installs should use an
explicit `codex_bin` override for manual SDK usage; the repo examples and
notebook bootstrap the pinned runtime package automatically.

## Maintainer workflow

```bash
cd sdk/python
python scripts/update_sdk_artifacts.py generate-types
python scripts/update_sdk_artifacts.py \
  stage-sdk \
  /tmp/codex-python-release/openai-codex-app-server-sdk \
  --codex-version <codex-release-tag-or-pep440-version>
python scripts/update_sdk_artifacts.py \
  stage-runtime \
  /tmp/codex-python-release/openai-codex-cli-bin \
  /path/to/codex \
  --codex-version <codex-release-tag-or-pep440-version>
```

Pass `--platform-tag ...` to `stage-runtime` when the wheel should be tagged for
a Rust target that differs from the Python build host. The intended one-off
matrix is `macosx_11_0_arm64`, `macosx_10_9_x86_64`,
`musllinux_1_1_aarch64`, `musllinux_1_1_x86_64`, `win_arm64`, and
`win_amd64`.

This supports the CI release flow:

- run `generate-types` before packaging
- stage `openai-codex-app-server-sdk` once with an exact `openai-codex-cli-bin==...` dependency
- stage `openai-codex-cli-bin` on each supported platform runner with the same pinned runtime version
- build and publish `openai-codex-cli-bin` as platform wheels only; do not publish an sdist

## Compatibility and versioning

- Package: `openai-codex-app-server-sdk`
- Runtime package: `openai-codex-cli-bin`
- Python: `>=3.10`
- Target protocol: Codex `app-server` JSON-RPC v2
- Versioning rule: the SDK package version is the underlying Codex runtime version

## Notes

- `Codex()` is eager and performs startup + `initialize` in the constructor.
- Use context managers (`with Codex() as codex:`) to ensure shutdown.
- Prefer `thread.run("...")` for the common case. Use `thread.turn(...)` when
  you need streaming, steering, or interrupt control.
- For transient overload, use `codex_app_server.retry.retry_on_overload`.
