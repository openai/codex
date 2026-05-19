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
    ),
    "aarch64-unknown-linux-musl": TargetSpec(
        target="aarch64-unknown-linux-musl",
        is_windows=False,
        is_linux=True,
    ),
    "x86_64-apple-darwin": TargetSpec(
        target="x86_64-apple-darwin",
        is_windows=False,
        is_linux=False,
    ),
    "aarch64-apple-darwin": TargetSpec(
        target="aarch64-apple-darwin",
        is_windows=False,
        is_linux=False,
    ),
    "x86_64-pc-windows-msvc": TargetSpec(
        target="x86_64-pc-windows-msvc",
        is_windows=True,
        is_linux=False,
    ),
    "aarch64-pc-windows-msvc": TargetSpec(
        target="aarch64-pc-windows-msvc",
        is_windows=True,
        is_linux=False,
    ),
}


def resolve_rg_bin(spec: TargetSpec, rg_bin: Path | None) -> Path:
    return resolve_input_path(
        rg_bin,
        default_rg_candidates(spec),
        "ripgrep executable",
        "--rg-bin",
    )


def default_rg_candidates(spec: TargetSpec) -> list[Path]:
    return [
        REPO_ROOT / "codex-cli" / "vendor" / spec.target / "path" / spec.rg_name,
    ]


def resolve_input_path(
    explicit_path: Path | None,
    default_candidates: list[Path],
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

    for candidate in default_candidates:
        if candidate.is_file():
            return candidate.resolve()

    candidates = "\n".join(f"  - {candidate}" for candidate in default_candidates)
    raise RuntimeError(
        f"Could not find {description}. Pass {flag_name}, or create one of:\n{candidates}"
    )


def is_executable(path: Path) -> bool:
    return bool(path.stat().st_mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH))
