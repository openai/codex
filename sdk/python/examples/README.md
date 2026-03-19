# Python SDK Examples

Each example folder contains runnable versions:

- `sync.py` (public sync surface: `Codex`)
- `async.py` (public async surface: `AsyncCodex`)

All examples intentionally use only public SDK exports from `codex_app_server`.

## Prerequisites

- Python `>=3.10`
- Install SDK dependencies for the same Python interpreter you will use to run examples

Recommended setup (from `sdk/python`):

```bash
python -m venv .venv
source .venv/bin/activate
python -m pip install -U pip
python -m pip install -e .
```

When running examples from this repo checkout, the SDK source uses the local
tree and does not bundle a runtime binary. The helper in `examples/_bootstrap.py`
uses the installed `codex-cli-bin` runtime package.

If the pinned `codex-cli-bin` runtime is not already installed, the bootstrap
will download the matching GitHub release artifact, stage a temporary local
`codex-cli-bin` package, install it into your active interpreter, and clean up
the temporary files afterward.

Current pinned runtime version: `0.116.0-alpha.1`

## Run examples

From `sdk/python`:

```bash
python examples/<example-folder>/sync.py
python examples/<example-folder>/async.py
```

The examples bootstrap local imports from `sdk/python/src` automatically, so no
SDK wheel install is required. You only need the Python dependencies for your
active interpreter and an installed `codex-cli-bin` runtime package (either
already present or automatically provisioned by the bootstrap).

## Recommended first run

```bash
python examples/01_quickstart_constructor/sync.py
python examples/01_quickstart_constructor/async.py
```

## Index

- `01_quickstart_constructor/`
  - first run / sanity check
- `02_turn_run/`
  - inspect full turn output fields
- `03_turn_stream_events/`
  - stream a turn with a small curated event view
- `04_models_and_metadata/`
  - discover visible models for the connected runtime
- `05_existing_thread/`
  - resume a real existing thread (created in-script)
- `06_thread_lifecycle_and_controls/`
  - thread lifecycle + control calls
- `07_image_and_text/`
  - remote image URL + text multimodal turn
- `08_local_image_and_text/`
  - local image + text multimodal turn using a generated temporary sample image
- `09_async_parity/`
  - parity-style sync flow (see async parity in other examples)
- `10_error_handling_and_retry/`
  - overload retry pattern + typed error handling structure
- `11_cli_mini_app/`
  - interactive chat loop
- `12_turn_params_kitchen_sink/`
  - structured output with a curated advanced `turn(...)` configuration
- `13_model_select_and_turn_params/`
  - list models, pick highest model + highest supported reasoning effort, run turns, print message and usage
- `14_turn_controls/`
  - separate best-effort `steer()` and `interrupt()` demos with concise summaries
