"""Cargo builds for source-built Codex package artifacts."""

import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

from .targets import REPO_ROOT
from .targets import PackageVariant
from .targets import TargetSpec


CODEX_RS_ROOT = REPO_ROOT / "codex-rs"


@dataclass(frozen=True)
class SourceBuildOutputs:
    entrypoint_bin: Path
    bwrap_bin: Path | None
    codex_command_runner_bin: Path | None
    codex_windows_sandbox_setup_bin: Path | None


def build_source_binaries(
    spec: TargetSpec,
    variant: PackageVariant,
    *,
    cargo: str,
    profile: str,
    entrypoint_bin: Path | None,
) -> SourceBuildOutputs:
    binaries = source_binaries_for_target(
        spec,
        variant,
        build_entrypoint=entrypoint_bin is None,
    )
    if binaries:
        cmd = [
            cargo,
            "build",
            "--target",
            spec.target,
            "--profile",
            profile,
        ]
        for binary in binaries:
            cmd.extend(["--bin", binary])

        print("+", " ".join(cmd))
        subprocess.run(cmd, cwd=CODEX_RS_ROOT, check=True)

    output_dir = cargo_profile_output_dir(spec, profile)
    outputs = SourceBuildOutputs(
        entrypoint_bin=(
            entrypoint_bin.resolve()
            if entrypoint_bin is not None
            else output_dir / variant.entrypoint_name(spec)
        ),
        bwrap_bin=output_dir / "bwrap" if spec.is_linux else None,
        codex_command_runner_bin=(
            output_dir / "codex-command-runner.exe" if spec.is_windows else None
        ),
        codex_windows_sandbox_setup_bin=(
            output_dir / "codex-windows-sandbox-setup.exe" if spec.is_windows else None
        ),
    )
    validate_source_outputs(outputs)
    return outputs


def source_binaries_for_target(
    spec: TargetSpec,
    variant: PackageVariant,
    *,
    build_entrypoint: bool,
) -> list[str]:
    binaries = []
    if build_entrypoint:
        binaries.append(variant.cargo_bin)
    if spec.is_linux:
        binaries.append("bwrap")
    if spec.is_windows:
        binaries.extend(
            [
                "codex-command-runner",
                "codex-windows-sandbox-setup",
            ]
        )
    return binaries


def cargo_profile_output_dir(spec: TargetSpec, profile: str) -> Path:
    target_dir = cargo_target_dir()
    return target_dir / spec.target / cargo_profile_dirname(profile)


def cargo_target_dir() -> Path:
    target_dir = os.environ.get("CARGO_TARGET_DIR")
    if target_dir is None:
        return CODEX_RS_ROOT / "target"

    path = Path(target_dir)
    if path.is_absolute():
        return path

    return CODEX_RS_ROOT / path


def cargo_profile_dirname(profile: str) -> str:
    if profile == "dev":
        return "debug"
    if profile == "release":
        return "release"
    return profile


def validate_source_outputs(outputs: SourceBuildOutputs) -> None:
    for path in [
        outputs.entrypoint_bin,
        outputs.bwrap_bin,
        outputs.codex_command_runner_bin,
        outputs.codex_windows_sandbox_setup_bin,
    ]:
        if path is not None and not path.is_file():
            raise RuntimeError(f"cargo build did not produce expected binary: {path}")
