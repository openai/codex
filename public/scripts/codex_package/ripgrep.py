"""Fetch ripgrep from the DotSlash manifest used by the package builder."""

from pathlib import Path

from .dotslash import fetch_dotslash_executable
from .targets import TargetSpec, resolve_input_path

RG_MANIFEST = Path(__file__).with_name("rg")


def resolve_rg_bin(spec: TargetSpec, rg_bin: Path | None) -> Path:
    if rg_bin is not None:
        return resolve_input_path(rg_bin, "ripgrep executable", "--rg-bin")

    return fetch_rg(spec)


def fetch_rg(
    spec: TargetSpec,
    *,
    manifest_path: Path = RG_MANIFEST,
) -> Path:
    rg_bin = fetch_dotslash_executable(
        spec,
        manifest_path=manifest_path,
        artifact_label="ripgrep",
        cache_key=f"{spec.target}-rg",
        dest_name=spec.rg_name,
    )
    if rg_bin is None:
        raise AssertionError("ripgrep is required for all package targets")
    return rg_bin
