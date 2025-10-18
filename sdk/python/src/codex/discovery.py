from __future__ import annotations

import platform
import sys
from pathlib import Path

from .exceptions import UnsupportedPlatformError


def _detect_target() -> str:
    system = sys.platform
    machine = platform.machine().lower()

    if system in {"linux", "linux2"}:
        if machine in {"x86_64", "amd64"}:
            return "x86_64-unknown-linux-musl"
        if machine in {"aarch64", "arm64"}:
            return "aarch64-unknown-linux-musl"
    elif system == "darwin":
        if machine == "x86_64":
            return "x86_64-apple-darwin"
        if machine in {"arm64", "aarch64"}:
            return "aarch64-apple-darwin"
    elif system == "win32":
        if machine in {"x86_64", "amd64"}:
            return "x86_64-pc-windows-msvc"
        if machine in {"arm64", "aarch64"}:
            return "aarch64-pc-windows-msvc"

    raise UnsupportedPlatformError(system, machine)


def find_codex_binary(override: str | None = None) -> Path:
    if override:
        return Path(override)

    target = _detect_target()
    package_root = Path(__file__).resolve().parent
    vendor_root = package_root / "vendor" / target / "codex"
    binary_name = "codex.exe" if sys.platform == "win32" else "codex"
    binary_path = vendor_root / binary_name
    return binary_path
