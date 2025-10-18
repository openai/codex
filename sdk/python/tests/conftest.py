from __future__ import annotations

import os
import subprocess
from pathlib import Path
from typing import Any, Callable, Dict, List

import pytest

from codex import Codex, CodexOptions
from codex.exec import INTERNAL_ORIGINATOR_ENV

PROJECT_ROOT = Path(__file__).resolve().parents[3]
BINARY_NAME = "codex.exe" if os.name == "nt" else "codex"
CODEX_BIN = PROJECT_ROOT / "codex-rs" / "target" / "debug" / BINARY_NAME


@pytest.fixture(autouse=True)
def _reset_originator_env() -> None:
    os.environ.pop(INTERNAL_ORIGINATOR_ENV, None)


@pytest.fixture(scope="session")
def codex_binary() -> Path:
    if not CODEX_BIN.exists():
        pytest.skip("codex binary not built at target/debug/codex")
    return CODEX_BIN


@pytest.fixture
def codex_client(codex_binary: Path) -> Callable[[str], Codex]:
    def _make(base_url: str) -> Codex:
        options = CodexOptions(
            codex_path_override=str(codex_binary),
            base_url=base_url,
            api_key="test",
        )
        return Codex(options)

    return _make


@pytest.fixture
def codex_exec_spy(monkeypatch: pytest.MonkeyPatch) -> List[Dict[str, Any]]:
    calls: List[Dict[str, Any]] = []
    original_popen = subprocess.Popen

    def spy_popen(command, *args, **kwargs):  # type: ignore[no-untyped-def]
        calls.append({"command": command, "args": args, "kwargs": kwargs})
        return original_popen(command, *args, **kwargs)

    monkeypatch.setattr(subprocess, "Popen", spy_popen)
    return calls
