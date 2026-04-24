from __future__ import annotations

from pathlib import Path

from codex_app_server_bin import PACKAGE_NAME
from codex_app_server_bin import bundled_app_server_path


def bundled_codex_path() -> Path:
    return bundled_app_server_path()


__all__ = ["PACKAGE_NAME", "bundled_app_server_path", "bundled_codex_path"]
