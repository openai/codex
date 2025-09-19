#!/usr/bin/env python3
"""Shared helpers for staging ripgrep binaries."""

from __future__ import annotations

import json
import os
import platform
import shutil
import subprocess
import tarfile
import tempfile
import zipfile
from pathlib import Path
from typing import Sequence
from urllib.parse import urlparse
from urllib.request import urlopen

SCRIPT_DIR = Path(__file__).resolve().parent
CODEX_CLI_ROOT = SCRIPT_DIR.parent
RG_MANIFEST = CODEX_CLI_ROOT / "bin" / "rg"

RG_TARGET_PLATFORM_PAIRS: list[tuple[str, str]] = [
    ("x86_64-unknown-linux-musl", "linux-x86_64"),
    ("aarch64-unknown-linux-musl", "linux-aarch64"),
    ("x86_64-apple-darwin", "macos-x86_64"),
    ("aarch64-apple-darwin", "macos-aarch64"),
    ("x86_64-pc-windows-msvc.exe", "windows-x86_64"),
    ("aarch64-pc-windows-msvc.exe", "windows-aarch64"),
]
RG_TARGET_TO_PLATFORM = {target: platform for target, platform in RG_TARGET_PLATFORM_PAIRS}
DEFAULT_RG_TARGETS = [target for target, _ in RG_TARGET_PLATFORM_PAIRS]


def detect_current_target() -> str | None:
    """Return the Codex target triple for the current platform, if known."""

    sys_platform = platform.system().lower()
    machine = platform.machine().lower()

    if sys_platform in {"linux", "linux2"}:
        if machine in {"x86_64", "amd64"}:
            return "x86_64-unknown-linux-musl"
        if machine in {"arm64", "aarch64"}:
            return "aarch64-unknown-linux-musl"
    elif sys_platform == "darwin":
        if machine == "x86_64":
            return "x86_64-apple-darwin"
        if machine in {"arm64", "aarch64"}:
            return "aarch64-apple-darwin"
    elif sys_platform.startswith("win"):
        if machine in {"x86_64", "amd64"}:
            return "x86_64-pc-windows-msvc.exe"
        if machine in {"arm64", "aarch64"}:
            return "aarch64-pc-windows-msvc.exe"
    return None


def fetch_rg(
    bin_dir: Path,
    targets: Sequence[str] | None = None,
    *,
    manifest_path: Path = RG_MANIFEST,
) -> list[Path]:
    """Download ripgrep binaries described by the DotSlash manifest.

    Args:
        bin_dir: Destination directory where the binaries will be written.
        targets: Optional iterable of Codex target triples. When omitted, all
            supported targets are fetched.
        manifest_path: Path to the DotSlash manifest describing providers.

    Returns:
        List of Paths to the downloaded binaries.
    """

    if targets is None:
        targets = DEFAULT_RG_TARGETS

    if not manifest_path.exists():
        raise FileNotFoundError(f"DotSlash manifest not found: {manifest_path}")

    manifest = json.loads(manifest_path.read_text())
    platforms = manifest.get("platforms", {})

    bin_dir.mkdir(parents=True, exist_ok=True)

    results: list[Path] = []
    for target in targets:
        platform_key = RG_TARGET_TO_PLATFORM.get(target)
        if platform_key is None:
            raise ValueError(f"Unsupported ripgrep target '{target}'.")

        platform_info = platforms.get(platform_key)
        if platform_info is None:
            raise RuntimeError(f"Platform '{platform_key}' not found in manifest {manifest_path}.")

        providers = platform_info.get("providers", [])
        if not providers:
            raise RuntimeError(
                f"No providers listed for platform '{platform_key}' in {manifest_path}."
            )

        url = providers[0]["url"]
        archive_format = platform_info.get("format", "zst")
        archive_path = platform_info.get("path")

        dest_name = f"rg-{target}"
        dest = bin_dir / dest_name
        if platform_key.startswith("win") and dest.suffix != ".exe":
            dest = dest.with_suffix(".exe")

        with tempfile.TemporaryDirectory() as tmp_dir_str:
            tmp_dir = Path(tmp_dir_str)
            archive_filename = os.path.basename(urlparse(url).path)
            download_path = tmp_dir / archive_filename
            _download_file(url, download_path)

            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.unlink(missing_ok=True)
            _extract_archive(download_path, archive_format, archive_path, dest)

        if not platform_key.startswith("win"):
            dest.chmod(0o755)

        results.append(dest)

    return results


def _download_file(url: str, dest: Path) -> None:
    """Download the content at url and write it to dest."""

    dest.parent.mkdir(parents=True, exist_ok=True)
    with urlopen(url) as response, open(dest, "wb") as out:
        shutil.copyfileobj(response, out)


def _extract_archive(
    archive_path: Path,
    archive_format: str,
    archive_member: str | None,
    dest: Path,
) -> None:
    """Extract `archive_member` from `archive_path` into `dest`."""

    dest.parent.mkdir(parents=True, exist_ok=True)

    if archive_format == "zst":
        output_path = archive_path.parent / dest.name
        subprocess.run(
            ["zstd", "-f", "-d", str(archive_path), "-o", str(output_path)],
            check=True,
        )
        shutil.move(str(output_path), dest)
        return

    if archive_format == "tar.gz":
        if not archive_member:
            raise RuntimeError("Missing 'path' for tar.gz archive in DotSlash manifest.")
        with tarfile.open(archive_path, "r:gz") as tar:
            try:
                member = tar.getmember(archive_member)
            except KeyError as exc:
                raise RuntimeError(
                    f"Entry '{archive_member}' not found in archive {archive_path}."
                ) from exc
            tar.extract(member, path=archive_path.parent)
        extracted = archive_path.parent / archive_member
        shutil.move(str(extracted), dest)
        return

    if archive_format == "zip":
        if not archive_member:
            raise RuntimeError("Missing 'path' for zip archive in DotSlash manifest.")
        with zipfile.ZipFile(archive_path) as archive:
            try:
                with archive.open(archive_member) as src, open(dest, "wb") as out:
                    shutil.copyfileobj(src, out)
            except KeyError as exc:
                raise RuntimeError(
                    f"Entry '{archive_member}' not found in archive {archive_path}."
                ) from exc
        return

    raise RuntimeError(f"Unsupported archive format '{archive_format}'.")


__all__ = [
    "DEFAULT_RG_TARGETS",
    "RG_TARGET_PLATFORM_PAIRS",
    "RG_TARGET_TO_PLATFORM",
    "detect_current_target",
    "fetch_rg",
]
