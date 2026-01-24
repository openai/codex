#!/usr/bin/env python3
"""Build wheel with bundled Codex CLI.

This script handles the complete wheel building process:
1. Optionally updates version
2. Downloads Codex CLI
3. Builds the wheel
4. Optionally cleans up the bundled CLI

Usage:
    python scripts/build_wheel.py                    # Build with current version
    python scripts/build_wheel.py --version 0.1.4    # Build with specific version
    python scripts/build_wheel.py --clean            # Clean bundled CLI after build
    python scripts/build_wheel.py --skip-download    # Skip CLI download (use existing)
"""

import argparse
import os
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path

try:
    import twine  # noqa: F401

    HAS_TWINE = True
except ImportError:
    HAS_TWINE = False


def run_command(cmd: list[str], description: str) -> None:
    """Run a command and handle errors."""
    print(f"\n{'=' * 60}")
    print(f"{description}")
    print(f"{'=' * 60}")
    print(f"$ {' '.join(cmd)}")
    print()

    try:
        result = subprocess.run(
            cmd,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        print(result.stdout)
    except subprocess.CalledProcessError as e:
        print(f"Error: {description} failed", file=sys.stderr)
        print(e.stdout, file=sys.stderr)
        sys.exit(1)


def update_version(version: str) -> None:
    """Update package version."""
    script_dir = Path(__file__).parent
    update_script = script_dir / "update_version.py"

    if not update_script.exists():
        print("Warning: update_version.py not found, skipping version update")
        return

    run_command(
        [sys.executable, str(update_script), version],
        f"Updating version to {version}",
    )


def get_bundled_cli_version() -> str:
    """Get the CLI version that should be bundled from _cli_version.py."""
    version_file = Path("src/agent_sdk/_cli_version.py")
    if not version_file.exists():
        return "latest"

    content = version_file.read_text()
    match = re.search(r'__cli_version__ = "([^"]+)"', content)
    if match:
        return match.group(1)
    return "latest"


def download_cli(cli_version: str | None = None) -> None:
    """Download Codex CLI."""
    # Use provided version, or fall back to version from _cli_version.py
    if cli_version is None:
        cli_version = get_bundled_cli_version()

    script_dir = Path(__file__).parent
    download_script = script_dir / "download_cli.py"

    # Set environment variable for download script
    os.environ["CODEX_CLI_VERSION"] = cli_version

    run_command(
        [sys.executable, str(download_script)],
        f"Downloading Codex CLI ({cli_version})",
    )


def clean_dist() -> None:
    """Clean dist directory."""
    dist_dir = Path("dist")
    if dist_dir.exists():
        print(f"\n{'=' * 60}")
        print("Cleaning dist directory")
        print(f"{'=' * 60}")
        shutil.rmtree(dist_dir)
        print("Cleaned dist/")


def get_platform_tag() -> str:
    """Get the appropriate platform tag for the current platform.

    Uses minimum compatible versions for broad compatibility:
    - macOS: 11.0 (Big Sur) as minimum
    - Linux: manylinux_2_17 (widely compatible)
    - Windows: Standard tags
    """
    system = platform.system()
    machine = platform.machine().lower()

    if system == "Darwin":
        # macOS - use minimum version 11.0 (Big Sur) for broad compatibility
        if machine == "arm64":
            return "macosx_11_0_arm64"
        else:
            return "macosx_11_0_x86_64"
    elif system == "Linux":
        # Linux - use manylinux for broad compatibility
        if machine in ["x86_64", "amd64"]:
            return "manylinux_2_17_x86_64"
        elif machine in ["aarch64", "arm64"]:
            return "manylinux_2_17_aarch64"
        else:
            return f"linux_{machine}"
    elif system == "Windows":
        # Windows
        if machine in ["x86_64", "amd64"]:
            return "win_amd64"
        elif machine == "arm64":
            return "win_arm64"
        else:
            return "win32"
    else:
        # Unknown platform, use generic
        return f"{system.lower()}_{machine}"


def retag_wheel(wheel_path: Path, platform_tag: str) -> Path:
    """Retag a wheel with the correct platform tag using wheel package."""
    print(f"\n{'=' * 60}")
    print("Retagging wheel as platform-specific")
    print(f"{'=' * 60}")
    print(f"Old: {wheel_path.name}")

    # Use wheel package to properly retag (updates both filename and metadata)
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "wheel",
            "tags",
            "--platform-tag",
            platform_tag,
            "--remove",
            str(wheel_path),
        ],
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print(f"Warning: Failed to retag wheel: {result.stderr}")
        return wheel_path

    # Find the newly tagged wheel
    dist_dir = wheel_path.parent
    # The wheel package creates a new file with the platform tag
    new_wheels = list(dist_dir.glob(f"*{platform_tag}.whl"))

    if new_wheels:
        new_path = new_wheels[0]
        print(f"New: {new_path.name}")
        print("Wheel retagged successfully")

        # Remove the old wheel
        if wheel_path.exists() and wheel_path != new_path:
            wheel_path.unlink()

        return new_path
    else:
        print("Warning: Could not find retagged wheel")
        return wheel_path


def build_wheel() -> None:
    """Build the wheel."""
    run_command(
        [sys.executable, "-m", "build", "--wheel"],
        "Building wheel",
    )

    # Check if we have a bundled CLI - if so, retag the wheel as platform-specific
    bundled_cli = Path("src/agent_sdk/_bundled/codex")
    bundled_cli_exe = Path("src/agent_sdk/_bundled/codex.exe")

    if bundled_cli.exists() or bundled_cli_exe.exists():
        # Find the built wheel
        dist_dir = Path("dist")
        wheels = list(dist_dir.glob("*.whl"))

        if wheels:
            # Get platform tag
            platform_tag = get_platform_tag()

            # Retag each wheel (should only be one)
            for wheel in wheels:
                if "-any.whl" in wheel.name:
                    retag_wheel(wheel, platform_tag)
        else:
            print("Warning: No wheel found to retag")
    else:
        print("\nNo bundled CLI found - wheel will be platform-independent")


def build_sdist() -> None:
    """Build the source distribution."""
    run_command(
        [sys.executable, "-m", "build", "--sdist"],
        "Building source distribution",
    )


def check_package() -> None:
    """Check package with twine."""
    if not HAS_TWINE:
        print("\nWarning: twine not installed, skipping package check")
        print("Install with: pip install twine")
        return

    print(f"\n{'=' * 60}")
    print("Checking package with twine")
    print(f"{'=' * 60}")
    print(f"$ {sys.executable} -m twine check dist/*")
    print()

    try:
        result = subprocess.run(
            [sys.executable, "-m", "twine", "check", "dist/*"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        print(result.stdout)

        if result.returncode != 0:
            print("\nWarning: twine check reported issues")
            print("Note: 'License-File' warnings are false positives from twine 6.x")
            print("PyPI will accept these packages without issues")
        else:
            print("Package check passed")
    except Exception as e:
        print(f"Warning: Failed to run twine check: {e}")


def clean_bundled_cli() -> None:
    """Clean bundled CLI."""
    bundled_dir = Path("src/agent_sdk/_bundled")
    cli_files = list(bundled_dir.glob("codex*"))

    if cli_files:
        print(f"\n{'=' * 60}")
        print("Cleaning bundled CLI")
        print(f"{'=' * 60}")
        for cli_file in cli_files:
            if cli_file.name != ".gitignore":
                cli_file.unlink()
                print(f"Removed {cli_file}")
    else:
        print("\nNo bundled CLI to clean")


def list_artifacts() -> None:
    """List built artifacts."""
    dist_dir = Path("dist")
    if not dist_dir.exists():
        return

    print(f"\n{'=' * 60}")
    print("Built Artifacts")
    print(f"{'=' * 60}")

    artifacts = sorted(dist_dir.iterdir())
    if not artifacts:
        print("No artifacts found")
        return

    for artifact in artifacts:
        size_mb = artifact.stat().st_size / (1024 * 1024)
        print(f"  {artifact.name:<50} {size_mb:>8.2f} MB")

    total_size = sum(f.stat().st_size for f in artifacts) / (1024 * 1024)
    print(f"\n  {'Total:':<50} {total_size:>8.2f} MB")


def main() -> None:
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Build wheel with bundled Codex CLI"
    )
    parser.add_argument(
        "--version",
        help="Version to set before building (e.g., 0.1.4)",
    )
    parser.add_argument(
        "--cli-version",
        default=None,
        help="Codex CLI version to download (default: read from _cli_version.py)",
    )
    parser.add_argument(
        "--skip-download",
        action="store_true",
        help="Skip downloading CLI (use existing bundled CLI)",
    )
    parser.add_argument(
        "--skip-sdist",
        action="store_true",
        help="Skip building source distribution",
    )
    parser.add_argument(
        "--clean",
        action="store_true",
        help="Clean bundled CLI after building",
    )
    parser.add_argument(
        "--clean-dist",
        action="store_true",
        help="Clean dist directory before building",
    )

    args = parser.parse_args()

    print("\n" + "=" * 60)
    print("Codex Agent SDK - Wheel Builder")
    print("=" * 60)

    # Clean dist if requested
    if args.clean_dist:
        clean_dist()

    # Update version if specified
    if args.version:
        update_version(args.version)

    # Download CLI unless skipped
    if not args.skip_download:
        download_cli(args.cli_version)
    else:
        print("\nSkipping CLI download (using existing)")

    # Build wheel
    build_wheel()

    # Build sdist unless skipped
    if not args.skip_sdist:
        build_sdist()

    # Check package
    check_package()

    # Clean bundled CLI if requested
    if args.clean:
        clean_bundled_cli()

    # List artifacts
    list_artifacts()

    print(f"\n{'=' * 60}")
    print("Build complete!")
    print(f"{'=' * 60}")
    print("\nNext steps:")
    print("  1. Test the wheel: pip install dist/*.whl")
    print("  2. Run tests: python -m pytest tests/")
    print("  3. Publish: twine upload dist/*")


if __name__ == "__main__":
    main()
