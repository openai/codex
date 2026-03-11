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

## Run examples

From `sdk/python`:

```bash
python examples/<example-folder>/sync.py
python examples/<example-folder>/async.py
```

The examples bootstrap local imports from `sdk/python/src` automatically, so no `pip install -e .` step is required to run them from this repository checkout.
The only required install step is dependencies for your active interpreter.

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
  - stream and print raw notifications
- `04_models_and_metadata/`
  - read server metadata and model list
- `05_existing_thread/`
  - resume a real existing thread (created in-script)
- `06_thread_lifecycle_and_controls/`
  - thread lifecycle + control calls
- `07_image_and_text/`
  - remote image URL + text multimodal turn
- `08_local_image_and_text/`
  - local image + text multimodal turn using bundled sample image
- `09_async_parity/`
  - parity-style sync flow (see async parity in other examples)
- `10_error_handling_and_retry/`
  - overload retry pattern + typed error handling structure
- `11_cli_mini_app/`
  - interactive chat loop
- `12_turn_params_kitchen_sink/`
  - one turn using most optional `turn(...)` params (sync + async)
- `13_model_select_and_turn_params/`
  - list models, pick highest model + highest supported reasoning effort, run turns, print message and usage
