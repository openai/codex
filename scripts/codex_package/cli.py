"""Command-line interface for building Codex package directories."""

import argparse
import tempfile
from pathlib import Path

from .archive import write_archive
from .cargo import build_source_binaries
from .layout import build_package_dir
from .layout import prepare_package_dir
from .layout import validate_package_dir
from .ripgrep import resolve_rg_bin
from .targets import PACKAGE_VARIANTS
from .targets import TARGET_SPECS
from .targets import PackageInputs
from .targets import default_target
from .targets import resolve_input_path
from .version import read_workspace_version


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build a canonical Codex package directory and optional archive.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "--target",
        default=argparse.SUPPRESS,
        choices=sorted(TARGET_SPECS),
        help=(
            "Rust target triple for the package. Defaults to the release target "
            "for this host platform."
        ),
    )
    parser.add_argument(
        "--variant",
        choices=sorted(PACKAGE_VARIANTS),
        default="codex",
        help="Package variant to build.",
    )
    parser.add_argument(
        "--package-dir",
        type=Path,
        default=argparse.SUPPRESS,
        help=(
            "Output directory to create as the package root. Defaults to a new "
            "temporary directory."
        ),
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
        "--entrypoint-bin",
        type=Path,
        help=(
            "Optional prebuilt entrypoint executable for the selected package "
            "variant. If omitted, the entrypoint is built with Cargo."
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
    spec = TARGET_SPECS[getattr(args, "target", None) or default_target()]
    variant = PACKAGE_VARIANTS[args.variant]
    package_dir_arg = getattr(args, "package_dir", None)
    package_dir = (
        package_dir_arg.resolve()
        if package_dir_arg is not None
        else Path(tempfile.mkdtemp(prefix="codex-package-")).resolve()
    )

    source_outputs = build_source_binaries(
        spec,
        variant,
        cargo=args.cargo,
        profile=args.cargo_profile,
        entrypoint_bin=(
            resolve_input_path(
                args.entrypoint_bin,
                "prebuilt entrypoint executable",
                "--entrypoint-bin",
            )
            if args.entrypoint_bin is not None
            else None
        ),
    )
    version = read_workspace_version()
    inputs = PackageInputs(
        entrypoint_bin=source_outputs.entrypoint_bin,
        rg_bin=resolve_rg_bin(spec, args.rg_bin),
        bwrap_bin=source_outputs.bwrap_bin,
        codex_command_runner_bin=source_outputs.codex_command_runner_bin,
        codex_windows_sandbox_setup_bin=source_outputs.codex_windows_sandbox_setup_bin,
    )
    prepare_package_dir(package_dir, force=args.force)
    build_package_dir(package_dir, version, variant, spec, inputs)
    validate_package_dir(package_dir, variant, spec)

    archive_output = args.archive_output
    if archive_output is not None:
        archive_path = archive_output.resolve()
        write_archive(package_dir, archive_path, force=args.force)
        print(f"Built Codex package archive at {archive_path}")

    print(f"Built Codex package directory at {package_dir}")
    return 0
