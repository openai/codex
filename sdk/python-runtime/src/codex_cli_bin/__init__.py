from __future__ import annotations

import os
from pathlib import Path

PACKAGE_NAME = "openai-codex-cli-bin"


def bundled_bin_dir() -> Path:
    return Path(__file__).resolve().parent / "bin"


def bundled_runtime_files() -> tuple[Path, ...]:
    names = (
        ("codex.exe", "codex-command-runner.exe", "codex-windows-sandbox-setup.exe")
        if os.name == "nt"
        else ("codex",)
    )
    return tuple(bundled_bin_dir() / name for name in names)


def bundled_codex_path() -> Path:
    exe = "codex.exe" if os.name == "nt" else "codex"
    path = bundled_bin_dir() / exe
    if not path.is_file():
        raise FileNotFoundError(
            f"{PACKAGE_NAME} is installed but missing its packaged codex binary at {path}"
        )
    return path


__all__ = [
    "PACKAGE_NAME",
    "bundled_bin_dir",
    "bundled_codex_path",
    "bundled_runtime_files",
]
