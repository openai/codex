from __future__ import annotations

import re
from importlib.metadata import PackageNotFoundError
from importlib.metadata import version as distribution_version
from pathlib import Path

DISTRIBUTION_NAME = "openai-codex-app-server-sdk"
UNKNOWN_VERSION = "0+unknown"


def package_version() -> str:
    source_version = _source_tree_project_version()
    if source_version is not None:
        return source_version

    try:
        return distribution_version(DISTRIBUTION_NAME)
    except PackageNotFoundError:
        return UNKNOWN_VERSION


def _source_tree_project_version() -> str | None:
    pyproject_path = Path(__file__).resolve().parents[2] / "pyproject.toml"
    if not pyproject_path.exists():
        return None

    match = re.search(
        r'(?m)^version = "([^"]+)"$',
        pyproject_path.read_text(encoding="utf-8"),
    )
    if match is None:
        return None
    return match.group(1)


__version__ = package_version()
