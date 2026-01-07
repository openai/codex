from __future__ import annotations

import os
from pathlib import Path

import pytest
from pydantic import BaseModel

from codex_sdk import Codex, ThreadRunError
from codex_sdk.errors import AuthRequiredError, CodexNotInstalledError

from .utils import fake_codex_path, read_log


class SummarySchema(BaseModel):
    summary: str
    status: str


@pytest.fixture
def log_path(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    path = tmp_path / "codex-log.jsonl"
    monkeypatch.setenv("CODEX_FAKE_LOG", str(path))
    monkeypatch.setenv("CODEX_FAKE_MODE", "basic")
    return path


def test_run_returns_items_and_usage(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()
    result = thread.run("Hello, world!")

    assert result.final_response == "Hi!"
    assert result.items == [{"id": "item_0", "type": "agent_message", "text": "Hi!"}]
    assert result.usage == {"input_tokens": 42, "cached_input_tokens": 12, "output_tokens": 5}
    assert thread.id is not None


def test_run_twice_passes_resume(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()
    thread.run("first input")
    thread.run("second input")

    entries = read_log(log_path)
    assert len(entries) >= 2
    first_args = entries[0]["args"]
    second_args = entries[1]["args"]

    assert "resume" not in first_args
    assert "resume" in second_args
    resume_index = second_args.index("resume")
    assert second_args[resume_index + 1] == thread.id


def test_resume_thread_uses_existing_id(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    original = client.start_thread()
    original.run("first input")

    resumed = client.resume_thread(original.id or "")
    result = resumed.run("second input")

    assert resumed.id == original.id
    assert result.final_response == "Hi!"


def test_passes_thread_options(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread(model="gpt-test-1", sandbox_mode="workspace-write")
    thread.run("apply options")

    entries = read_log(log_path)
    args = entries[0]["args"]
    assert "--sandbox" in args
    assert args[args.index("--sandbox") + 1] == "workspace-write"
    assert "--model" in args
    assert args[args.index("--model") + 1] == "gpt-test-1"


def test_passes_model_reasoning_effort(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread(model_reasoning_effort="high")
    thread.run("apply reasoning effort")

    args = read_log(log_path)[0]["args"]
    assert "--config" in args
    assert 'model_reasoning_effort="high"' in args


def test_passes_network_access_enabled(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread(network_access_enabled=True)
    thread.run("test network access")

    args = read_log(log_path)[0]["args"]
    assert "--config" in args
    assert "sandbox_workspace_write.network_access=true" in args


def test_passes_web_search_enabled(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread(web_search_enabled=True)
    thread.run("test web search")

    args = read_log(log_path)[0]["args"]
    assert "--config" in args
    assert "features.web_search_request=true" in args


def test_passes_approval_policy(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread(approval_policy="on-request")
    thread.run("test approval policy")

    args = read_log(log_path)[0]["args"]
    assert "--config" in args
    assert 'approval_policy="on-request"' in args


def test_env_override_does_not_leak(monkeypatch: pytest.MonkeyPatch, log_path: Path) -> None:
    monkeypatch.setenv("CODEX_ENV_SHOULD_NOT_LEAK", "leak")

    client = Codex(
        codex_path_override=fake_codex_path(),
        base_url="http://test",
        api_key="test",
        env={
            "CUSTOM_ENV": "custom",
            "CODEX_FAKE_LOG": str(log_path),
            "CODEX_FAKE_MODE": "basic",
        },
    )
    thread = client.start_thread()
    thread.run("custom env")

    env = read_log(log_path)[0]["env"]
    assert env.get("CUSTOM_ENV") == "custom"
    assert "CODEX_ENV_SHOULD_NOT_LEAK" not in env
    assert env.get("OPENAI_BASE_URL") == "http://test"
    assert env.get("CODEX_API_KEY") == "test"
    assert env.get("CODEX_INTERNAL_ORIGINATOR_OVERRIDE") is not None


def test_additional_directories(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread(additional_directories=["../backend", "/tmp/shared"])
    thread.run("test additional dirs")

    args = read_log(log_path)[0]["args"]
    add_dir_values = []
    for i, arg in enumerate(args):
        if arg == "--add-dir" and i + 1 < len(args):
            add_dir_values.append(args[i + 1])
    assert add_dir_values == ["../backend", "/tmp/shared"]


def test_output_schema_file_cleanup(log_path: Path) -> None:
    schema = {
        "type": "object",
        "properties": {"answer": {"type": "string"}},
        "required": ["answer"],
        "additionalProperties": False,
    }

    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()
    thread.run("structured", output_schema=schema)

    args = read_log(log_path)[0]["args"]
    assert "--output-schema" in args
    schema_path = args[args.index("--output-schema") + 1]
    assert schema_path
    assert not Path(schema_path).exists()


def test_output_schema_from_pydantic(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()
    thread.run("structured", output_schema=SummarySchema)

    args = read_log(log_path)[0]["args"]
    assert "--output-schema" in args


def test_combines_text_segments(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()
    thread.run([
        {"type": "text", "text": "Describe file changes"},
        {"type": "text", "text": "Focus on impacted tests"},
    ])

    stdin_text = read_log(log_path)[0]["stdin"]
    assert stdin_text == "Describe file changes\n\nFocus on impacted tests"


def test_forwards_images(log_path: Path, tmp_path: Path) -> None:
    image_1 = tmp_path / "first.png"
    image_2 = tmp_path / "second.jpg"
    image_1.write_text("image-0")
    image_2.write_text("image-1")

    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()
    thread.run(
        [
            {"type": "text", "text": "describe the images"},
            {"type": "local_image", "path": str(image_1)},
            {"type": "local_image", "path": str(image_2)},
        ]
    )

    args = read_log(log_path)[0]["args"]
    forwarded = []
    for i, arg in enumerate(args):
        if arg == "--image" and i + 1 < len(args):
            forwarded.append(args[i + 1])
    assert forwarded == [str(image_1), str(image_2)]


def test_working_directory_requires_git(tmp_path: Path, log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread(working_directory=str(tmp_path))

    with pytest.raises(ThreadRunError, match="Not inside a trusted directory"):
        thread.run("use custom working directory")


def test_missing_codex_raises(tmp_path: Path) -> None:
    missing = tmp_path / "missing-codex"
    client = Codex(codex_path_override=str(missing), base_url="http://test", api_key="test")
    thread = client.start_thread()

    with pytest.raises(CodexNotInstalledError):
        thread.run("fail")


def test_missing_auth_raises(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    log_path = tmp_path / "auth-log.jsonl"
    monkeypatch.setenv("CODEX_FAKE_LOG", str(log_path))
    monkeypatch.setenv("CODEX_FAKE_MODE", "basic")

    client = Codex(
        codex_path_override=fake_codex_path(),
        base_url="http://test",
        env={"CODEX_HOME": str(tmp_path / "codex-home")},
    )
    thread = client.start_thread()

    with pytest.raises(AuthRequiredError):
        thread.run("needs auth")
