#!/usr/bin/env python3
"""Stage and optionally package the @openai/codex npm module."""

from __future__ import annotations

import argparse
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
CODEX_CLI_ROOT = SCRIPT_DIR.parent
REPO_ROOT = CODEX_CLI_ROOT.parent


def _load_rg_utils():
    spec = importlib.util.spec_from_file_location("rg_utils", SCRIPT_DIR / "rg_utils.py")
    if spec is None or spec.loader is None:
        raise RuntimeError("Unable to load rg_utils module")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


rg_utils = _load_rg_utils()
DEFAULT_RG_TARGETS = rg_utils.DEFAULT_RG_TARGETS
fetch_rg = rg_utils.fetch_rg


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build the Codex CLI npm package.")
    parser.add_argument(
        "--version",
        required=True,
        help="Version number to write to package.json inside the staged package.",
    )
    parser.add_argument(
        "--workflow-url",
        help="Optional GitHub Actions workflow run URL used to download native binaries.",
    )
    parser.add_argument(
        "--staging-dir",
        type=Path,
        help=(
            "Directory to stage the package contents. Defaults to a new temporary directory "
            "if omitted. The directory must be empty when provided."
        ),
    )
    parser.add_argument(
        "--pack-output",
        type=Path,
        help="Path where the generated npm tarball should be written.",
    )
    parser.add_argument(
        "--rg-target",
        action="append",
        dest="rg_targets",
        help="Codex target triple for which ripgrep should be fetched. Repeatable.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    staging_dir, created_temp = prepare_staging_dir(args.staging_dir)

    try:
        stage_sources(staging_dir, args.version)
        install_native_binaries(staging_dir, args.workflow_url)

        rg_targets: Sequence[str] | None = args.rg_targets
        if rg_targets is None:
            rg_targets = DEFAULT_RG_TARGETS
        fetch_rg(staging_dir / "bin", rg_targets)

        print(f"Staged package in {staging_dir}")

        if args.pack_output is not None:
            output_path = run_npm_pack(staging_dir, args.pack_output)
            print(f"npm pack output written to {output_path}")
    finally:
        if created_temp:
            # Preserve the staging directory for further inspection.
            pass

    return 0


def prepare_staging_dir(staging_dir: Path | None) -> tuple[Path, bool]:
    if staging_dir is not None:
        staging_dir = staging_dir.resolve()
        staging_dir.mkdir(parents=True, exist_ok=True)
        if any(staging_dir.iterdir()):
            raise RuntimeError(f"Staging directory {staging_dir} is not empty.")
        return staging_dir, False

    temp_dir = Path(tempfile.mkdtemp(prefix="codex-npm-stage-"))
    return temp_dir, True


def stage_sources(staging_dir: Path, version: str) -> None:
    bin_dir = staging_dir / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)

    shutil.copy2(CODEX_CLI_ROOT / "bin" / "codex.js", bin_dir / "codex.js")
    rg_manifest = CODEX_CLI_ROOT / "bin" / "rg"
    if rg_manifest.exists():
        shutil.copy2(rg_manifest, bin_dir / "rg")

    readme_src = REPO_ROOT / "README.md"
    if readme_src.exists():
        shutil.copy2(readme_src, staging_dir / "README.md")

    with open(CODEX_CLI_ROOT / "package.json", "r", encoding="utf-8") as fh:
        package_json = json.load(fh)
    package_json["version"] = version

    with open(staging_dir / "package.json", "w", encoding="utf-8") as out:
        json.dump(package_json, out, indent=2)
        out.write("\n")


def install_native_binaries(staging_dir: Path, workflow_url: str | None) -> None:
    cmd = ["./scripts/install_native_deps.sh"]
    if workflow_url:
        cmd.extend(["--workflow-url", workflow_url])
    cmd.append(str(staging_dir))
    subprocess.run(cmd, cwd=CODEX_CLI_ROOT, check=True)


def run_npm_pack(staging_dir: Path, output_path: Path) -> Path:
    output_path = output_path.resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="codex-npm-pack-") as pack_dir_str:
        pack_dir = Path(pack_dir_str)
        result = subprocess.run(
            ["npm", "pack", "--json", "--pack-destination", str(pack_dir)],
            cwd=staging_dir,
            check=True,
            text=True,
            capture_output=True,
        )

        try:
            pack_output = json.loads(result.stdout)
        except json.JSONDecodeError as exc:
            raise RuntimeError("Failed to parse npm pack output.") from exc

        if not pack_output:
            raise RuntimeError("npm pack did not produce an output tarball.")

        tarball_name = pack_output[0].get("filename") or pack_output[0].get("name")
        if not tarball_name:
            raise RuntimeError("Unable to determine npm pack output filename.")

        tarball_path = pack_dir / tarball_name
        if not tarball_path.exists():
            raise RuntimeError(f"Expected npm pack output not found: {tarball_path}")

        shutil.move(str(tarball_path), output_path)

    return output_path


if __name__ == "__main__":
    sys.exit(main())
