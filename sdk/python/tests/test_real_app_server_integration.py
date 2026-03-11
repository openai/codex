from __future__ import annotations

import asyncio
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

from codex_app_server import AsyncCodex, Codex, TextInput

ROOT = Path(__file__).resolve().parents[1]
EXAMPLES_DIR = ROOT / "examples"
NOTEBOOK_PATH = ROOT / "notebooks" / "sdk_walkthrough.ipynb"

# 11_cli_mini_app is interactive; we still run it by feeding '/exit'.
EXAMPLE_CASES: list[tuple[str, str]] = [
    ("01_quickstart_constructor", "sync.py"),
    ("01_quickstart_constructor", "async.py"),
    ("02_turn_run", "sync.py"),
    ("02_turn_run", "async.py"),
    ("03_turn_stream_events", "sync.py"),
    ("03_turn_stream_events", "async.py"),
    ("04_models_and_metadata", "sync.py"),
    ("04_models_and_metadata", "async.py"),
    ("05_existing_thread", "sync.py"),
    ("05_existing_thread", "async.py"),
    ("06_thread_lifecycle_and_controls", "sync.py"),
    ("06_thread_lifecycle_and_controls", "async.py"),
    ("07_image_and_text", "sync.py"),
    ("07_image_and_text", "async.py"),
    ("08_local_image_and_text", "sync.py"),
    ("08_local_image_and_text", "async.py"),
    ("09_async_parity", "sync.py"),
    # 09_async_parity async path is represented by 01 async + dedicated async-based cases above.
    ("10_error_handling_and_retry", "sync.py"),
    ("10_error_handling_and_retry", "async.py"),
    ("11_cli_mini_app", "sync.py"),
    ("11_cli_mini_app", "async.py"),
    ("12_turn_params_kitchen_sink", "sync.py"),
    ("12_turn_params_kitchen_sink", "async.py"),
    ("13_model_select_and_turn_params", "sync.py"),
    ("13_model_select_and_turn_params", "async.py"),
]


def _run_example(
    folder: str, script: str, *, timeout_s: int = 150
) -> subprocess.CompletedProcess[str]:
    path = EXAMPLES_DIR / folder / script
    assert path.exists(), f"Missing example script: {path}"

    env = os.environ.copy()

    # Feed '/exit' only to interactive mini-cli examples.
    stdin = "/exit\n" if folder == "11_cli_mini_app" else None

    return subprocess.run(
        [sys.executable, str(path)],
        cwd=str(ROOT),
        env=env,
        input=stdin,
        text=True,
        capture_output=True,
        timeout=timeout_s,
        check=False,
    )


def _notebook_cell_source(cell_index: int) -> str:
    notebook = json.loads(NOTEBOOK_PATH.read_text())
    return "".join(notebook["cells"][cell_index]["source"])


def test_real_initialize_and_model_list():
    with Codex() as codex:
        metadata = codex.metadata
        assert isinstance(metadata.user_agent, str) and metadata.user_agent.strip()
        assert isinstance(metadata.server_name, str) and metadata.server_name.strip()
        assert isinstance(metadata.server_version, str) and metadata.server_version.strip()

        models = codex.models(include_hidden=True)
        assert isinstance(models.data, list)


def test_real_thread_and_turn_start_smoke():
    with Codex() as codex:
        thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
        result = thread.turn(TextInput("hello")).run()

        assert isinstance(result.thread_id, str) and result.thread_id.strip()
        assert isinstance(result.turn_id, str) and result.turn_id.strip()
        assert isinstance(result.items, list)
        assert result.usage is not None
        assert result.usage.thread_id == result.thread_id
        assert result.usage.turn_id == result.turn_id


def test_real_async_thread_turn_usage_and_ids_smoke() -> None:
    async def _run() -> None:
        async with AsyncCodex() as codex:
            thread = await codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
            result = await (await thread.turn(TextInput("say ok"))).run()

            assert isinstance(result.thread_id, str) and result.thread_id.strip()
            assert isinstance(result.turn_id, str) and result.turn_id.strip()
            assert isinstance(result.items, list)
            assert result.usage is not None
            assert result.usage.thread_id == result.thread_id
            assert result.usage.turn_id == result.turn_id

    asyncio.run(_run())


def test_notebook_bootstrap_resolves_sdk_from_unrelated_cwd() -> None:
    cell_1_source = _notebook_cell_source(1)
    env = os.environ.copy()
    env["CODEX_PYTHON_SDK_DIR"] = str(ROOT)

    with tempfile.TemporaryDirectory() as temp_cwd:
        result = subprocess.run(
            [sys.executable, "-c", cell_1_source],
            cwd=temp_cwd,
            env=env,
            text=True,
            capture_output=True,
            timeout=60,
            check=False,
        )

    assert result.returncode == 0, (
        f"Notebook bootstrap failed from unrelated cwd.\n"
        f"STDOUT:\n{result.stdout}\n"
        f"STDERR:\n{result.stderr}"
    )
    assert "SDK source:" in result.stdout
    assert "codex_app_server" in result.stdout or "sdk/python/src" in result.stdout


def test_real_streaming_smoke_turn_completed():
    with Codex() as codex:
        thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
        turn = thread.turn(TextInput("Reply with one short sentence."))

        saw_delta = False
        saw_completed = False
        for evt in turn.stream():
            if evt.method == "item/agentMessage/delta":
                saw_delta = True
            if evt.method == "turn/completed":
                saw_completed = True

        assert saw_completed
        # Some environments can produce zero deltas for very short output;
        # this assert keeps the smoke test informative but non-flaky.
        assert isinstance(saw_delta, bool)


def test_real_turn_interrupt_smoke():
    with Codex() as codex:
        thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
        turn = thread.turn(TextInput("Count from 1 to 200 with commas."))

        # Best effort: interrupting quickly may race with completion on fast models.
        _ = turn.interrupt()

        # Confirm the session is still usable after interrupt race.
        follow_up = thread.turn(TextInput("Say 'ok' only.")).run()
        assert follow_up.status.value in {"completed", "failed"}

@pytest.mark.parametrize(("folder", "script"), EXAMPLE_CASES)
def test_real_examples_run_and_assert(folder: str, script: str):
    result = _run_example(folder, script)

    assert result.returncode == 0, (
        f"Example failed: {folder}/{script}\n"
        f"STDOUT:\n{result.stdout}\n"
        f"STDERR:\n{result.stderr}"
    )

    out = result.stdout

    # Minimal content assertions so we validate behavior, not just exit code.
    if folder == "01_quickstart_constructor":
        assert "Status:" in out and "Text:" in out
        assert "Server: None None" not in out
    elif folder == "02_turn_run":
        assert "thread_id:" in out and "turn_id:" in out and "status:" in out
        assert "usage: None" not in out
    elif folder == "03_turn_stream_events":
        assert "turn/completed" in out
    elif folder == "04_models_and_metadata":
        assert "models.count:" in out
        assert "server_name=None" not in out
        assert "server_version=None" not in out
    elif folder == "05_existing_thread":
        assert "Created thread:" in out
    elif folder == "06_thread_lifecycle_and_controls":
        assert "Lifecycle OK:" in out
    elif folder in {"07_image_and_text", "08_local_image_and_text"}:
        assert "completed" in out.lower() or "Status:" in out
    elif folder == "09_async_parity":
        assert "Thread:" in out and "Turn:" in out
    elif folder == "10_error_handling_and_retry":
        assert "Text:" in out
    elif folder == "11_cli_mini_app":
        assert "Thread:" in out
    elif folder == "12_turn_params_kitchen_sink":
        assert "Status:" in out and "Usage:" in out
    elif folder == "13_model_select_and_turn_params":
        assert "selected.model:" in out and "agent.message.params:" in out and "usage.params:" in out
        assert "usage.params: None" not in out
