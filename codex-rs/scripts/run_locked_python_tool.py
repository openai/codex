"""Run a tool with dependencies verified against its uv.lock."""

import argparse
import os
import subprocess
from pathlib import Path
from tempfile import TemporaryDirectory


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", required=True)
    parser.add_argument("--only-group")
    parser.add_argument("tool", choices=("python", "ruff"))
    parser.add_argument("tool_args", nargs=argparse.REMAINDER)
    args = parser.parse_args()

    uv = ["uv", "--project", args.project]
    group_args = ["--only-group", args.only_group] if args.only_group else []
    requirements = subprocess.check_output(
        [*uv, "export", "--frozen", "--no-emit-project", *group_args]
    )
    with TemporaryDirectory(prefix="codex-python-tool-") as temp:
        venv = str(Path(temp) / ".venv")
        subprocess.run([*uv, "venv", venv], check=True)
        subprocess.run(
            [*uv, "pip", "sync", "--require-hashes", "--python", venv, "-"],
            input=requirements,
            check=True,
        )
        tool = args.tool
        if tool == "ruff":
            # Fail if the locked Ruff executable is missing from the environment.
            tool = str(
                Path(venv) / ("Scripts/ruff.exe" if os.name == "nt" else "bin/ruff")
            )
        return subprocess.call(
            [*uv, "run", "--no-sync", "--active", "--", tool, *args.tool_args],
            env={**os.environ, "VIRTUAL_ENV": venv},
        )


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.returncode) from None
