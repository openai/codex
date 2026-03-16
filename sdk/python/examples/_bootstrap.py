from __future__ import annotations

import importlib.util
import os
import sys
from pathlib import Path
from typing import Iterable

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


def server_label(metadata: object) -> str:
    server = getattr(metadata, "serverInfo", None)
    server_name = ((getattr(server, "name", None) or "") if server is not None else "").strip()
    server_version = ((getattr(server, "version", None) or "") if server is not None else "").strip()
    if server_name and server_version:
        return f"{server_name} {server_version}"

    user_agent = ((getattr(metadata, "userAgent", None) or "") if metadata is not None else "").strip()
    return user_agent or "unknown"


def find_turn_by_id(turns: Iterable[object] | None, turn_id: str) -> object | None:
    for turn in turns or []:
        if getattr(turn, "id", None) == turn_id:
            return turn
    return None


def assistant_text_from_turn(turn: object | None) -> str:
    if turn is None:
        return ""

    chunks: list[str] = []
    for item in getattr(turn, "items", []) or []:
        raw_item = item.model_dump(mode="json") if hasattr(item, "model_dump") else item
        if not isinstance(raw_item, dict):
            continue

        item_type = raw_item.get("type")
        if item_type == "agentMessage":
            text = raw_item.get("text")
            if isinstance(text, str) and text:
                chunks.append(text)
            continue

        if item_type != "message" or raw_item.get("role") != "assistant":
            continue

        for content in raw_item.get("content") or []:
            if not isinstance(content, dict) or content.get("type") != "output_text":
                continue
            text = content.get("text")
            if isinstance(text, str) and text:
                chunks.append(text)

    return "".join(chunks)
