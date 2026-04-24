from __future__ import annotations

import os
from pathlib import Path

PACKAGE_NAME = "openai-codex-app-server-bin"


def bundled_app_server_path() -> Path:
    package_root = Path(__file__).resolve().parent
    for exe in _candidate_binary_names():
        path = package_root / "bin" / exe
        if path.is_file():
            return path

    candidate_list = ", ".join(
        str(package_root / "bin" / exe) for exe in _candidate_binary_names()
    )
    raise FileNotFoundError(
        f"{PACKAGE_NAME} is installed but missing its packaged app-server binary. "
        f"Checked: {candidate_list}"
    )


def _candidate_binary_names() -> tuple[str, str]:
    if os.name == "nt":
        return ("codex-app-server.exe", "codex.exe")
    return ("codex-app-server", "codex")


__all__ = ["PACKAGE_NAME", "bundled_app_server_path"]
