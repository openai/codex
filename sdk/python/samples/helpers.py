from __future__ import annotations

import os
from pathlib import Path


def codex_path_override() -> str:
    env_override = os.environ.get("CODEX_EXECUTABLE")
    if env_override:
        return env_override

    return str(Path.cwd() / ".." / ".." / "codex-rs" / "target" / "debug" / "codex")
