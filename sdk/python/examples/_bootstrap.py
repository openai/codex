from __future__ import annotations

import importlib.util
import os
import sys
from pathlib import Path

_SDK_PYTHON_DIR = Path(__file__).resolve().parents[1]
_SDK_PYTHON_STR = str(_SDK_PYTHON_DIR)
if _SDK_PYTHON_STR not in sys.path:
    sys.path.insert(0, _SDK_PYTHON_STR)

from _runtime_setup import ensure_runtime_package_installed


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
    sdk_python_dir = _SDK_PYTHON_DIR
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
    """Return an example-friendly AppServerConfig for repo-source SDK usage."""
    from codex_app_server import AppServerConfig

    ensure_runtime_package_installed(sys.executable, _SDK_PYTHON_DIR)
    return AppServerConfig()
