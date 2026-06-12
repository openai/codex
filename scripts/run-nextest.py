"""Build runtime helpers and run the Codex nextest suite."""

import os
import subprocess
import sys
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parent))

from codex_package.targets import REPO_ROOT
from codex_package.targets import TARGET_SPECS
from codex_package.v8 import resolve_codex_v8_cargo_env


CODEX_RS_ROOT = REPO_ROOT / "codex-rs"


def rustc_host_target() -> str:
    rustc = os.environ.get("RUSTC", "rustc")
    version = subprocess.run(
        [rustc, "-vV"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    for line in version.splitlines():
        if line.startswith("host: "):
            return line.removeprefix("host: ")
    raise RuntimeError("rustc -vV did not report a host target")


def main() -> int:
    target = rustc_host_target()
    try:
        spec = TARGET_SPECS[target]
    except KeyError as exc:
        raise RuntimeError(
            f"unsupported host target for code-mode tests: {target}"
        ) from exc

    env = {**os.environ, **resolve_codex_v8_cargo_env(spec)}
    cargo = os.environ.get("CARGO", "cargo")
    subprocess.run(
        [
            cargo,
            "build",
            "-p",
            "codex-code-mode-host",
            "--bin",
            "codex-code-mode-host",
        ],
        cwd=CODEX_RS_ROOT,
        check=True,
        env=env,
    )
    return subprocess.run(
        [cargo, "nextest", "run", "--no-fail-fast", *sys.argv[1:]],
        cwd=CODEX_RS_ROOT,
        env=env,
    ).returncode


if __name__ == "__main__":
    raise SystemExit(main())
