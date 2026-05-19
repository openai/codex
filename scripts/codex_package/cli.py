"""Command-line interface for building Codex package directories."""

import argparse
from pathlib import Path

from .archive import write_archive
from .cargo import build_source_binaries
from .layout import build_package_dir
from .layout import prepare_package_dir
from .layout import validate_package_dir
from .ripgrep import resolve_rg_bin
from .targets import TARGET_SPECS
from .targets import PackageInputs


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build a canonical Codex package directory and optional archive.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "--target",
        required=True,
        choices=sorted(TARGET_SPECS),
        help="Rust target triple for the package.",
    )
    parser.add_argument(
        "--version",
        default="0.0.0-dev",
        help="Codex version to record in codex-package.json.",
    )
    parser.add_argument(
        "--variant",
        default="codex",
        help="Package variant to record in codex-package.json.",
    )
    parser.add_argument(
        "--package-dir",
        type=Path,
        required=True,
        help="Output directory to create as the package root.",
    )
    parser.add_argument(
        "--archive-output",
        type=Path,
        help=(
            "Optional archive output path. Supported suffixes: .tar.gz, .tgz, "
            ".tar.zst, .zip."
        ),
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Replace an existing package directory or archive output.",
    )
    parser.add_argument(
        "--cargo",
        default="cargo",
        help="Cargo executable to use for source-built package artifacts.",
    )
    parser.add_argument(
        "--cargo-profile",
        default="dev-small",
        help=(
            "Cargo profile for source-built package artifacts. Use release for "
            "release packages."
        ),
    )
    parser.add_argument(
        "--rg-bin",
        type=Path,
        help=(
            "Optional local ripgrep executable override instead of fetching from "
            "codex-cli/bin/rg."
        ),
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    spec = TARGET_SPECS[args.target]
    package_dir = args.package_dir.resolve()

    source_outputs = build_source_binaries(
        spec,
        cargo=args.cargo,
        profile=args.cargo_profile,
    )
    inputs = PackageInputs(
        codex_bin=source_outputs.codex_bin,
        rg_bin=resolve_rg_bin(spec, args.rg_bin),
        bwrap_bin=source_outputs.bwrap_bin,
        codex_command_runner_bin=source_outputs.codex_command_runner_bin,
        codex_windows_sandbox_setup_bin=source_outputs.codex_windows_sandbox_setup_bin,
    )
    prepare_package_dir(package_dir, force=args.force)
    build_package_dir(package_dir, args.version, args.variant, spec, inputs)
    validate_package_dir(package_dir, spec)

    archive_output = args.archive_output
    if archive_output is not None:
        archive_path = archive_output.resolve()
        write_archive(package_dir, archive_path, force=args.force)
        print(f"Built Codex package archive at {archive_path}")

    print(f"Built Codex package directory at {package_dir}")
    return 0
