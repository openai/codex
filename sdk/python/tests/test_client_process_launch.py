from __future__ import annotations

from typing import Any

import pytest

import codex_app_server.client as client_module
from codex_app_server.client import AppServerClient, AppServerConfig


def test_start_launches_subprocess_with_utf8_text_io(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    launched: dict[str, Any] = {}

    class FakeProc:
        stdin = None
        stdout = None
        stderr = None

    def fake_popen(args: list[str], **kwargs: Any) -> FakeProc:
        launched["args"] = args
        launched["kwargs"] = kwargs
        return FakeProc()

    monkeypatch.setattr(client_module.subprocess, "Popen", fake_popen)

    client = AppServerClient(
        config=AppServerConfig(
            launch_args_override=("codex", "app-server", "--listen", "stdio://")
        )
    )

    client.start()

    assert launched["args"] == ["codex", "app-server", "--listen", "stdio://"]
    assert launched["kwargs"]["text"] is True
    assert launched["kwargs"]["encoding"] == "utf-8"
