from __future__ import annotations

import os
from pathlib import Path

PACKAGE_NAME = "codex-cli-bin"


def bundled_codex_path() -> Path:
    exe = "codex.exe" if os.name == "nt" else "codex"
    path = Path(__file__).resolve().parent / "bin" / exe
    if not path.is_file():
        raise FileNotFoundError(
            f"{PACKAGE_NAME} is installed but missing its packaged codex binary at {path}"
        )
    return path


__all__ = ["PACKAGE_NAME", "bundled_codex_path"]
