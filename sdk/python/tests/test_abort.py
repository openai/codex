from __future__ import annotations

import threading
import time
from pathlib import Path

import pytest

from codex_sdk import AbortController, Codex
from codex_sdk.errors import AbortError
from .utils import fake_codex_path


@pytest.fixture
def infinite_env(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    log_path = tmp_path / "abort-log.jsonl"
    monkeypatch.setenv("CODEX_FAKE_LOG", str(log_path))
    monkeypatch.setenv("CODEX_FAKE_MODE", "infinite")


def test_abort_before_run(infinite_env: None) -> None:
    controller = AbortController()
    controller.abort("stop")

    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()

    with pytest.raises(AbortError):
        thread.run("Hello", signal=controller.signal)


def test_abort_before_run_streamed(infinite_env: None) -> None:
    controller = AbortController()
    controller.abort("stop")

    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()

    streamed = thread.run_streamed("Hello", signal=controller.signal)
    with pytest.raises(AbortError):
        list(streamed.events)


def test_abort_during_run(infinite_env: None) -> None:
    controller = AbortController()

    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()

    timer = threading.Timer(0.05, lambda: controller.abort("stop"))
    timer.start()
    with pytest.raises(AbortError):
        thread.run("Hello", signal=controller.signal)
    timer.cancel()


def test_abort_during_run_streamed(infinite_env: None) -> None:
    controller = AbortController()

    client = Codex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()

    streamed = thread.run_streamed("Hello", signal=controller.signal)

    with pytest.raises(AbortError):
        for idx, _event in enumerate(streamed.events):
            if idx == 3:
                controller.abort("stop")
            time.sleep(0.01)
