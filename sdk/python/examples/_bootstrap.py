from __future__ import annotations

import importlib.util
import os
import shutil
import sys
from pathlib import Path


def _ensure_runtime_dependencies(sdk_python_dir: Path) -> None:
    if importlib.util.find_spec("pydantic") is not None:
        return

    python = sys.executable
    raise RuntimeError(
        "Missing required dependency: pydantic.\n"
        f"Interpreter: {python}\n"
        "Install dependencies with the same interpreter used to run this example:\n"
        f"  {python} -m pip install -e {sdk_python_dir}\n"
        "If you installed with `pip` from another Python, reinstall using the command above."
    )


def ensure_local_sdk_src() -> Path:
    """Add sdk/python/src to sys.path so examples run without installing the package."""
    sdk_python_dir = Path(__file__).resolve().parents[1]
    src_dir = sdk_python_dir / "src"
    package_dir = src_dir / "codex_app_server"
    if not package_dir.exists():
        raise RuntimeError(f"Could not locate local SDK package at {package_dir}")

    _ensure_runtime_dependencies(sdk_python_dir)

    src_str = str(src_dir)
    if src_str not in sys.path:
        sys.path.insert(0, src_str)
    return src_dir


def runtime_config():
    """Return an example-friendly AppServerConfig for local repo usage."""
    from codex_app_server import AppServerConfig

    codex_bin = os.environ.get("CODEX_PYTHON_SDK_CODEX_BIN") or shutil.which("codex")
    if codex_bin is None:
        raise RuntimeError(
            "Examples require a Codex CLI binary when run from this repo checkout.\n"
            "Set CODEX_PYTHON_SDK_CODEX_BIN=/absolute/path/to/codex, or ensure `codex` is on PATH."
        )
    return AppServerConfig(codex_bin=codex_bin)
