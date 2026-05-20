"""Supported package targets and default binary discovery."""

import stat
from dataclasses import dataclass
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parents[1]
REPO_ROOT = SCRIPT_DIR.parent


@dataclass(frozen=True)
class TargetSpec:
    target: str
    is_windows: bool
    is_linux: bool
    dotslash_platform: str

    @property
    def exe_suffix(self) -> str:
        return ".exe" if self.is_windows else ""

    @property
    def codex_name(self) -> str:
        return f"codex{self.exe_suffix}"

    @property
    def rg_name(self) -> str:
        return f"rg{self.exe_suffix}"


@dataclass(frozen=True)
class PackageInputs:
    codex_bin: Path
    rg_bin: Path
    bwrap_bin: Path | None
    codex_command_runner_bin: Path | None
    codex_windows_sandbox_setup_bin: Path | None


TARGET_SPECS: dict[str, TargetSpec] = {
    "x86_64-unknown-linux-musl": TargetSpec(
        target="x86_64-unknown-linux-musl",
        is_windows=False,
        is_linux=True,
        dotslash_platform="linux-x86_64",
    ),
    "aarch64-unknown-linux-musl": TargetSpec(
        target="aarch64-unknown-linux-musl",
        is_windows=False,
        is_linux=True,
        dotslash_platform="linux-aarch64",
    ),
    "x86_64-apple-darwin": TargetSpec(
        target="x86_64-apple-darwin",
        is_windows=False,
        is_linux=False,
        dotslash_platform="macos-x86_64",
    ),
    "aarch64-apple-darwin": TargetSpec(
        target="aarch64-apple-darwin",
        is_windows=False,
        is_linux=False,
        dotslash_platform="macos-aarch64",
    ),
    "x86_64-pc-windows-msvc": TargetSpec(
        target="x86_64-pc-windows-msvc",
        is_windows=True,
        is_linux=False,
        dotslash_platform="windows-x86_64",
    ),
    "aarch64-pc-windows-msvc": TargetSpec(
        target="aarch64-pc-windows-msvc",
        is_windows=True,
        is_linux=False,
        dotslash_platform="windows-aarch64",
    ),
}


def resolve_input_path(
    explicit_path: Path | None,
    description: str,
    flag_name: str,
) -> Path:
    if explicit_path is not None:
        path = explicit_path.resolve()
        if not path.is_file():
            raise RuntimeError(f"{description} does not exist: {path}")
        if not is_executable(path):
            raise RuntimeError(f"{description} is not executable: {path}")
        return path

    raise RuntimeError(f"Must specify {flag_name} for {description}.")


def is_executable(path: Path) -> bool:
    return bool(path.stat().st_mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH))
