import os
import subprocess
import sys
from pathlib import Path


def test_mcp_conformance_fixture_self_tests() -> None:
    repository_root = Path(__file__).resolve().parents[3]
    candidates = (
        repository_root / "public" / "scripts" / "mcp_conformance",
        repository_root / "scripts" / "mcp_conformance",
    )
    fixture_directory = next((candidate for candidate in candidates if candidate.is_dir()), None)
    assert fixture_directory is not None, (
        "MCP conformance fixtures are missing; checked "
        + " and ".join(str(candidate) for candidate in candidates)
    )

    env = os.environ.copy()
    env["PYTEST_DISABLE_PLUGIN_AUTOLOAD"] = "1"
    result = subprocess.run(
        [sys.executable, "-m", "pytest", "-q", str(fixture_directory)],
        cwd=repository_root,
        env=env,
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )

    assert result.returncode == 0, (
        f"MCP conformance fixture self-tests failed with exit "
        f"{result.returncode}.\n"
        f"Fixture directory: {fixture_directory}\n"
        f"STDOUT:\n{result.stdout}\n"
        f"STDERR:\n{result.stderr}"
    )
