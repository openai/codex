from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def fake_codex_path() -> str:
    return str(Path(__file__).parent / "support" / "fake_codex.py")


def read_log(path: Path) -> list[dict[str, Any]]:
    entries: list[dict[str, Any]] = []
    if not path.exists():
        return entries
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            entries.append(json.loads(line))
    return entries
