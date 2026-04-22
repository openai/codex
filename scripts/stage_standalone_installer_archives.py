#!/usr/bin/env python3
"""Stage standalone installer archives for Codex releases."""

from __future__ import annotations

import argparse
import importlib.util
import shutil
import tarfile
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
BUILD_SCRIPT = REPO_ROOT / "codex-cli" / "scripts" / "build_npm_package.py"

_SPEC = importlib.util.spec_from_file_location("codex_build_npm_package", BUILD_SCRIPT)
if _SPEC is None or _SPEC.loader is None:
    raise RuntimeError(f"Unable to load module from {BUILD_SCRIPT}")
_BUILD_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_BUILD_MODULE)
CODEX_PLATFORM_PACKAGES = getattr(_BUILD_MODULE, "CODEX_PLATFORM_PACKAGES", {})


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--release-version",
        required=True,
        help="Version to stage (e.g. 0.1.0 or 0.1.0-alpha.1).",
    )
    parser.add_argument(
        "--vendor-src",
        type=Path,
        required=True,
        help="Directory containing native binaries under vendor/<target>.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=REPO_ROOT / "dist" / "installer",
        help="Directory where standalone archives should be written.",
    )
    parser.add_argument(
        "--package",
        dest="packages",
        action="append",
        choices=sorted(CODEX_PLATFORM_PACKAGES),
        help=(
            "Codex platform package to stage. May be provided multiple times. "
            "Defaults to all platform packages."
        ),
    )
    return parser.parse_args()


def archive_name(platform_tag: str, version: str) -> str:
    return f"codex-standalone-{platform_tag}-{version}.tar.gz"


def copy_executable(source: Path, destination: Path) -> None:
    if not source.exists():
        raise RuntimeError(f"Missing standalone installer archive input: {source}")

    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    destination.chmod(0o755)


def stage_target(vendor_src: Path, staging_dir: Path, target: str, is_windows: bool) -> None:
    target_root = vendor_src / target
    codex_root = target_root / "codex"
    path_root = target_root / "path"
    resources_dir = staging_dir / "codex-resources"
    resources_dir.mkdir(parents=True, exist_ok=True)

    if is_windows:
        copy_executable(codex_root / "codex.exe", staging_dir / "codex.exe")
        copy_executable(
            codex_root / "codex-command-runner.exe",
            resources_dir / "codex-command-runner.exe",
        )
        copy_executable(
            codex_root / "codex-windows-sandbox-setup.exe",
            resources_dir / "codex-windows-sandbox-setup.exe",
        )
        copy_executable(path_root / "rg.exe", resources_dir / "rg.exe")
        return

    copy_executable(codex_root / "codex", staging_dir / "codex")
    copy_executable(path_root / "rg", resources_dir / "rg")


def write_archive(staging_dir: Path, output_path: Path) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(output_path, "w:gz") as archive:
        for path in sorted(staging_dir.rglob("*")):
            archive.add(path, arcname=path.relative_to(staging_dir), recursive=False)


def main() -> int:
    args = parse_args()
    vendor_src = args.vendor_src.resolve()
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    packages = args.packages or sorted(CODEX_PLATFORM_PACKAGES)
    for package in sorted(set(packages)):
        package_config = CODEX_PLATFORM_PACKAGES[package]
        platform_tag = package_config["npm_tag"]
        target = package_config["target_triple"]
        is_windows = package_config["os"] == "win32"
        output_path = output_dir / archive_name(platform_tag, args.release_version)

        with tempfile.TemporaryDirectory(
            prefix=f"codex-standalone-{platform_tag}-"
        ) as staging_dir_str:
            staging_dir = Path(staging_dir_str)
            stage_target(vendor_src, staging_dir, target, is_windows)
            write_archive(staging_dir, output_path)

        print(f"Staged standalone installer archive at {output_path}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
