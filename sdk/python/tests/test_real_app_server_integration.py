from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import textwrap
from dataclasses import dataclass
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
EXAMPLES_DIR = ROOT / "examples"
NOTEBOOK_PATH = ROOT / "notebooks" / "sdk_walkthrough.ipynb"

root_str = str(ROOT)
if root_str not in sys.path:
    sys.path.insert(0, root_str)

from _runtime_setup import ensure_runtime_package_installed, pinned_runtime_version

RUN_REAL_CODEX_TESTS = os.environ.get("RUN_REAL_CODEX_TESTS") == "1"
pytestmark = pytest.mark.skipif(
    not RUN_REAL_CODEX_TESTS,
    reason="set RUN_REAL_CODEX_TESTS=1 to run real Codex integration coverage",
)

# 11_cli_mini_app is interactive; we still run it by feeding one prompt, then '/exit'.
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
    ("14_turn_controls", "sync.py"),
    ("14_turn_controls", "async.py"),
]


@dataclass(frozen=True)
class PreparedRuntimeEnv:
    python: str
    env: dict[str, str]
    runtime_version: str


@pytest.fixture(scope="session")
def runtime_env(tmp_path_factory: pytest.TempPathFactory) -> PreparedRuntimeEnv:
    runtime_version = pinned_runtime_version()
    temp_root = tmp_path_factory.mktemp("python-runtime-env")
    isolated_site = temp_root / "site-packages"
    python = sys.executable

    _run_command(
        [
            python,
            "-m",
            "pip",
            "install",
            "--target",
            str(isolated_site),
            "pydantic>=2.12",
        ],
        cwd=ROOT,
        env=os.environ.copy(),
        timeout_s=240,
    )
    ensure_runtime_package_installed(
        python,
        ROOT,
        install_target=isolated_site,
    )

    env = os.environ.copy()
    env["PYTHONPATH"] = os.pathsep.join([str(isolated_site), str(ROOT / "src")])
    env["CODEX_PYTHON_SDK_DIR"] = str(ROOT)
    return PreparedRuntimeEnv(python=python, env=env, runtime_version=runtime_version)


def _run_command(
    args: list[str],
    *,
    cwd: Path,
    env: dict[str, str],
    timeout_s: int,
    stdin: str | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=str(cwd),
        env=env,
        input=stdin,
        text=True,
        capture_output=True,
        timeout=timeout_s,
        check=False,
    )


def _run_python(
    runtime_env: PreparedRuntimeEnv,
    source: str,
    *,
    cwd: Path | None = None,
    timeout_s: int = 180,
) -> subprocess.CompletedProcess[str]:
    return _run_command(
        [str(runtime_env.python), "-c", source],
        cwd=cwd or ROOT,
        env=runtime_env.env,
        timeout_s=timeout_s,
    )


def _runtime_compatibility_hint(
    runtime_env: PreparedRuntimeEnv,
    *,
    stdout: str,
    stderr: str,
) -> str:
    combined = f"{stdout}\n{stderr}"
    if "ThreadStartResponse" in combined and "approvalsReviewer" in combined:
        return (
            "\nCompatibility hint:\n"
            f"Pinned runtime {runtime_env.runtime_version} returned a thread/start payload "
            "that is older than the current SDK schema and is missing "
            "`approvalsReviewer`. Bump `sdk/python/_runtime_setup.py` to a matching "
            "released runtime version.\n"
        )
    return ""


def _run_json_python(
    runtime_env: PreparedRuntimeEnv,
    source: str,
    *,
    cwd: Path | None = None,
    timeout_s: int = 180,
) -> dict[str, object]:
    result = _run_python(runtime_env, source, cwd=cwd, timeout_s=timeout_s)
    assert result.returncode == 0, (
        "Python snippet failed.\n"
        f"STDOUT:\n{result.stdout}\n"
        f"STDERR:\n{result.stderr}"
        f"{_runtime_compatibility_hint(runtime_env, stdout=result.stdout, stderr=result.stderr)}"
    )
    return json.loads(result.stdout)


def _run_example(
    runtime_env: PreparedRuntimeEnv,
    folder: str,
    script: str,
    *,
    timeout_s: int = 180,
) -> subprocess.CompletedProcess[str]:
    path = EXAMPLES_DIR / folder / script
    assert path.exists(), f"Missing example script: {path}"

    stdin = (
        "Give 3 short bullets on SIMD.\nNow rewrite that as 1 short sentence.\n/exit\n"
        if folder == "11_cli_mini_app"
        else None
    )
    return _run_command(
        [str(runtime_env.python), str(path)],
        cwd=ROOT,
        env=runtime_env.env,
        timeout_s=timeout_s,
        stdin=stdin,
    )


def _notebook_cell_source(cell_index: int) -> str:
    notebook = json.loads(NOTEBOOK_PATH.read_text())
    return "".join(notebook["cells"][cell_index]["source"])


def test_real_initialize_and_model_list(runtime_env: PreparedRuntimeEnv) -> None:
    data = _run_json_python(
        runtime_env,
        textwrap.dedent(
            """
            import json
            from codex_app_server import Codex

            with Codex() as codex:
                models = codex.models(include_hidden=True)
                server = codex.metadata.serverInfo
                print(json.dumps({
                    "user_agent": codex.metadata.userAgent,
                    "server_name": None if server is None else server.name,
                    "server_version": None if server is None else server.version,
                    "model_count": len(models.data),
                }))
            """
        ),
    )

    assert isinstance(data["user_agent"], str) and data["user_agent"].strip()
    if data["server_name"] is not None:
        assert isinstance(data["server_name"], str) and data["server_name"].strip()
    if data["server_version"] is not None:
        assert isinstance(data["server_version"], str) and data["server_version"].strip()
    assert isinstance(data["model_count"], int)


def test_real_thread_and_turn_start_smoke(runtime_env: PreparedRuntimeEnv) -> None:
    data = _run_json_python(
        runtime_env,
        textwrap.dedent(
            """
            import json
            from codex_app_server import Codex, TextInput

            with Codex() as codex:
                thread = codex.thread_start(
                    model="gpt-5.4",
                    config={"model_reasoning_effort": "high"},
                )
                result = thread.turn(TextInput("hello")).run()
                persisted = thread.read(include_turns=True)
                persisted_turn = next(
                    (turn for turn in persisted.thread.turns or [] if turn.id == result.id),
                    None,
                )
                print(json.dumps({
                    "thread_id": thread.id,
                    "turn_id": result.id,
                    "status": result.status.value,
                    "items_count": len(result.items or []),
                    "persisted_items_count": 0 if persisted_turn is None else len(persisted_turn.items or []),
                }))
            """
        ),
    )

    assert isinstance(data["thread_id"], str) and data["thread_id"].strip()
    assert isinstance(data["turn_id"], str) and data["turn_id"].strip()
    assert data["status"] == "completed"
    assert isinstance(data["items_count"], int)
    assert isinstance(data["persisted_items_count"], int)


def test_real_async_thread_turn_usage_and_ids_smoke(
    runtime_env: PreparedRuntimeEnv,
) -> None:
    data = _run_json_python(
        runtime_env,
        textwrap.dedent(
            """
            import asyncio
            import json
            from codex_app_server import AsyncCodex, TextInput

            async def main():
                async with AsyncCodex() as codex:
                    thread = await codex.thread_start(
                        model="gpt-5.4",
                        config={"model_reasoning_effort": "high"},
                    )
                    result = await (await thread.turn(TextInput("say ok"))).run()
                    persisted = await thread.read(include_turns=True)
                    persisted_turn = next(
                        (turn for turn in persisted.thread.turns or [] if turn.id == result.id),
                        None,
                    )
                    print(json.dumps({
                        "thread_id": thread.id,
                        "turn_id": result.id,
                        "status": result.status.value,
                        "items_count": len(result.items or []),
                        "persisted_items_count": 0 if persisted_turn is None else len(persisted_turn.items or []),
                    }))

            asyncio.run(main())
            """
        ),
    )

    assert isinstance(data["thread_id"], str) and data["thread_id"].strip()
    assert isinstance(data["turn_id"], str) and data["turn_id"].strip()
    assert data["status"] == "completed"
    assert isinstance(data["items_count"], int)
    assert isinstance(data["persisted_items_count"], int)


def test_notebook_bootstrap_resolves_sdk_and_runtime_from_unrelated_cwd(
    runtime_env: PreparedRuntimeEnv,
) -> None:
    cell_1_source = _notebook_cell_source(1)
    env = runtime_env.env.copy()

    with tempfile.TemporaryDirectory() as temp_cwd:
        result = _run_command(
            [str(runtime_env.python), "-c", cell_1_source],
            cwd=Path(temp_cwd),
            env=env,
            timeout_s=180,
        )

    assert result.returncode == 0, (
        f"Notebook bootstrap failed from unrelated cwd.\n"
        f"STDOUT:\n{result.stdout}\n"
        f"STDERR:\n{result.stderr}"
    )
    assert "SDK source:" in result.stdout
    assert f"Runtime package: {runtime_env.runtime_version}" in result.stdout


def test_notebook_sync_cell_smoke(runtime_env: PreparedRuntimeEnv) -> None:
    source = "\n\n".join(
        [
            _notebook_cell_source(1),
            _notebook_cell_source(2),
            _notebook_cell_source(3),
        ]
    )
    result = _run_python(runtime_env, source, timeout_s=240)
    assert result.returncode == 0, (
        f"Notebook sync smoke failed.\nSTDOUT:\n{result.stdout}\nSTDERR:\n{result.stderr}"
    )
    assert "status:" in result.stdout
    assert "server:" in result.stdout


def test_notebook_advanced_cell_smoke(runtime_env: PreparedRuntimeEnv) -> None:
    source = "\n\n".join(
        [
            _notebook_cell_source(1),
            _notebook_cell_source(2),
            _notebook_cell_source(7),
        ]
    )
    result = _run_python(runtime_env, source, timeout_s=360)
    assert result.returncode == 0, (
        f"Notebook advanced smoke failed.\nSTDOUT:\n{result.stdout}\nSTDERR:\n{result.stderr}"
    )
    assert "selected.model:" in result.stdout
    assert "agent.message.params:" in result.stdout
    assert "items.params:" in result.stdout


def test_real_streaming_smoke_turn_completed(runtime_env: PreparedRuntimeEnv) -> None:
    data = _run_json_python(
        runtime_env,
        textwrap.dedent(
            """
            import json
            from codex_app_server import Codex, TextInput

            with Codex() as codex:
                thread = codex.thread_start(
                    model="gpt-5.4",
                    config={"model_reasoning_effort": "high"},
                )
                turn = thread.turn(TextInput("Reply with one short sentence."))
                saw_delta = False
                saw_completed = False
                for event in turn.stream():
                    if event.method == "item/agentMessage/delta":
                        saw_delta = True
                    if event.method == "turn/completed":
                        saw_completed = True
                print(json.dumps({
                    "saw_delta": saw_delta,
                    "saw_completed": saw_completed,
                }))
            """
        ),
    )

    assert data["saw_completed"] is True
    assert isinstance(data["saw_delta"], bool)


def test_real_turn_interrupt_smoke(runtime_env: PreparedRuntimeEnv) -> None:
    data = _run_json_python(
        runtime_env,
        textwrap.dedent(
            """
            import json
            from codex_app_server import Codex, TextInput

            with Codex() as codex:
                thread = codex.thread_start(
                    model="gpt-5.4",
                    config={"model_reasoning_effort": "high"},
                )
                turn = thread.turn(TextInput("Count from 1 to 200 with commas."))
                turn.interrupt()
                follow_up = thread.turn(TextInput("Say 'ok' only.")).run()
                print(json.dumps({"status": follow_up.status.value}))
            """
        ),
    )

    assert data["status"] in {"completed", "failed"}


@pytest.mark.parametrize(("folder", "script"), EXAMPLE_CASES)
def test_real_examples_run_and_assert(
    runtime_env: PreparedRuntimeEnv,
    folder: str,
    script: str,
) -> None:
    result = _run_example(runtime_env, folder, script)

    assert result.returncode == 0, (
        f"Example failed: {folder}/{script}\n"
        f"STDOUT:\n{result.stdout}\n"
        f"STDERR:\n{result.stderr}"
        f"{_runtime_compatibility_hint(runtime_env, stdout=result.stdout, stderr=result.stderr)}"
    )

    out = result.stdout

    if folder == "01_quickstart_constructor":
        assert "Status:" in out and "Text:" in out
        assert "Server: unknown" not in out
    elif folder == "02_turn_run":
        assert "thread_id:" in out and "turn_id:" in out and "status:" in out
        assert "persisted.items.count:" in out
    elif folder == "03_turn_stream_events":
        assert "stream.completed:" in out
        assert "assistant>" in out
    elif folder == "04_models_and_metadata":
        assert "server:" in out
        assert "models.count:" in out
        assert "models:" in out
        assert "metadata:" not in out
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
        assert out.count("assistant>") >= 2
        assert out.count("assistant.status>") >= 2
        assert out.count("usage>") >= 2
    elif folder == "12_turn_params_kitchen_sink":
        assert "Status:" in out
        assert "summary:" in out
        assert "actions:" in out
        assert "Items:" in out
    elif folder == "13_model_select_and_turn_params":
        assert "selected.model:" in out and "agent.message.params:" in out and "items.params:" in out
    elif folder == "14_turn_controls":
        assert "steer.result:" in out and "steer.final.status:" in out
        assert "interrupt.result:" in out and "interrupt.final.status:" in out
