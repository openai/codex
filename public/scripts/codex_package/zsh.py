"""Fetch the patched zsh fork used by shell_zsh_fork."""

from pathlib import Path

from .dotslash import fetch_dotslash_executable
from .targets import TargetSpec, resolve_input_path

ZSH_MANIFEST = Path(__file__).with_name("codex-zsh")
ZSH_RESOURCE_PATH = Path("zsh") / "bin" / "zsh"


def resolve_zsh_bin(
    spec: TargetSpec,
    manifest_path: Path | None = None,
    *,
    zsh_bin: Path | None = None,
) -> Path | None:
    if zsh_bin is not None:
        return resolve_input_path(zsh_bin, "zsh executable", "--zsh-bin")

    return fetch_dotslash_executable(
        spec,
        manifest_path=manifest_path or ZSH_MANIFEST,
        artifact_label="codex-zsh",
        cache_key=f"{spec.target}-zsh",
        dest_name="zsh",
        missing_ok=True,
    )
