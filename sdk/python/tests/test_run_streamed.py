from __future__ import annotations

from pathlib import Path

import pytest

from codex_sdk import Codex
from .utils import fake_codex_path, read_log


@pytest.fixture
def log_path(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    path = tmp_path / "codex-log.jsonl"
    monkeypatch.setenv("CODEX_FAKE_LOG", str(path))
    monkeypatch.setenv("CODEX_FAKE_MODE", "basic")
    return path


def test_run_streamed_returns_events(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()
    streamed = thread.run_streamed("Hello, world!")

    events = list(streamed.events)

    assert events == [
        {"type": "thread.started", "thread_id": thread.id},
        {"type": "turn.started"},
        {
            "type": "item.completed",
            "item": {"id": "item_0", "type": "agent_message", "text": "Hi!"},
        },
        {
            "type": "turn.completed",
            "usage": {"input_tokens": 42, "cached_input_tokens": 12, "output_tokens": 5},
        },
    ]


def test_run_streamed_twice_passes_resume(log_path: Path) -> None:
    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()

    first = thread.run_streamed("first input")
    list(first.events)

    second = thread.run_streamed("second input")
    list(second.events)

    entries = read_log(log_path)
    assert len(entries) >= 2
    assert "resume" in entries[1]["args"]


def test_run_streamed_output_schema(log_path: Path) -> None:
    schema = {
        "type": "object",
        "properties": {"answer": {"type": "string"}},
        "required": ["answer"],
        "additionalProperties": False,
    }

    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()
    streamed = thread.run_streamed("structured", output_schema=schema)
    list(streamed.events)

    args = read_log(log_path)[0]["args"]
    assert "--output-schema" in args
