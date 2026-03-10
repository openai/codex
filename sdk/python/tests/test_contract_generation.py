from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GENERATED_TARGETS = [
    Path("src/codex_app_server/generated/notification_registry.py"),
    Path("src/codex_app_server/generated/v2_all.py"),
    Path("src/codex_app_server/public_api.py"),
]


def _snapshot_target(root: Path, rel_path: Path) -> dict[str, bytes] | bytes | None:
    target = root / rel_path
    if not target.exists():
        return None
    if target.is_file():
        return target.read_bytes()

    snapshot: dict[str, bytes] = {}
    for path in sorted(target.rglob("*")):
        if path.is_file() and "__pycache__" not in path.parts:
            snapshot[str(path.relative_to(target))] = path.read_bytes()
    return snapshot


def _snapshot_targets(root: Path) -> dict[str, dict[str, bytes] | bytes | None]:
    return {
        str(rel_path): _snapshot_target(root, rel_path) for rel_path in GENERATED_TARGETS
    }


def test_generated_files_are_up_to_date():
    before = _snapshot_targets(ROOT)

    # Regenerate contract artifacts via single maintenance entrypoint.
    env = os.environ.copy()
    python_bin = str(Path(sys.executable).parent)
    env["PATH"] = f"{python_bin}{os.pathsep}{env.get('PATH', '')}"

    subprocess.run(
        [sys.executable, "scripts/update_sdk_artifacts.py", "--types-only"],
        cwd=ROOT,
        check=True,
        env=env,
    )

    after = _snapshot_targets(ROOT)
    assert before == after, "Generated files drifted after regeneration"
