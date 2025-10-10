#!/usr/bin/env python3
"""Install Codex native binaries for the Python SDK."""

from __future__ import annotations

import argparse
import shutil
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
INSTALL_NATIVE_DEPS = REPO_ROOT / "codex-cli" / "scripts" / "install_native_deps.py"
PYTHON_SDK_ROOT = REPO_ROOT / "sdk" / "python"
PACKAGE_ROOT = PYTHON_SDK_ROOT / "src" / "codex"
VENDOR_DIR = PACKAGE_ROOT / "vendor"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--workflow-url",
        help=(
            "GitHub Actions workflow URL containing the prebuilt Codex binaries. "
            "If omitted, the default from install_native_deps.py is used."
        ),
    )
    parser.add_argument(
        "--component",
        dest="components",
        action="append",
        default=["codex"],
        choices=("codex", "rg", "codex-responses-api-proxy"),
        help="Native component(s) to install (default: codex).",
    )
    parser.add_argument(
        "--clean",
        action="store_true",
        help="Remove the existing vendor directory before installing binaries.",
    )
    return parser.parse_args()


def ensure_install_script() -> None:
    if not INSTALL_NATIVE_DEPS.exists():
        raise FileNotFoundError(f"install_native_deps.py not found at {INSTALL_NATIVE_DEPS}")


def run_install(workflow_url: str | None, components: list[str]) -> None:
    cmd = [str(INSTALL_NATIVE_DEPS)]

    if workflow_url:
        cmd.extend(["--workflow-url", workflow_url])

    for component in components:
        cmd.extend(["--component", component])

    cmd.append(str(PACKAGE_ROOT))

    subprocess.run(cmd, check=True, cwd=REPO_ROOT)


def clean_vendor() -> None:
    if VENDOR_DIR.exists():
        shutil.rmtree(VENDOR_DIR)


def main() -> int:
    args = parse_args()
    ensure_install_script()

    if args.clean:
        clean_vendor()

    run_install(args.workflow_url, args.components)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
