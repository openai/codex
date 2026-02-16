from __future__ import annotations

import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_generated_files_are_up_to_date():
    # Regenerate contract artifacts.
    subprocess.run(["python3", "scripts/generate_types_from_schema.py"], cwd=ROOT, check=True)
    subprocess.run(["python3", "scripts/generate_protocol_typed_dicts.py"], cwd=ROOT, check=True)

    # Ensure no diff in generated targets after regeneration.
    diff = subprocess.run(
        [
            "git",
            "diff",
            "--",
            "src/codex_app_server/schema_types.py",
            "src/codex_app_server/protocol_types.py",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert diff.returncode == 0, f"Generated files drifted:\n{diff.stdout}\n{diff.stderr}"
