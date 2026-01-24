#!/usr/bin/env python3
"""Download Codex CLI binary for bundling in wheel.

This script is run during the wheel build process to fetch the Codex CLI
binary and place it in the package directory.
"""

import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path


def get_cli_version() -> str:
    """Get the CLI version to download from environment or default."""
    return os.environ.get("CODEX_CLI_VERSION", "latest")


def find_installed_cli() -> Path | None:
    """Find the installed Codex CLI binary."""
    system = platform.system()

    if system == "Windows":
        # Windows installation locations
        locations = [
            Path.home() / ".local" / "bin" / "codex.exe",
            Path(os.environ.get("LOCALAPPDATA", "")) / "Codex" / "codex.exe",
        ]
    else:
        # Unix installation locations
        locations = [
            Path.home() / ".local" / "bin" / "codex",
            Path("/usr/local/bin/codex"),
            Path.home() / "node_modules" / ".bin" / "codex",
        ]

    # Also check PATH
    cli_path = shutil.which("codex")
    if cli_path:
        return Path(cli_path)

    for path in locations:
        if path.exists() and path.is_file():
            return path

    return None


def download_cli() -> None:
    """Download Codex CLI."""
    version = get_cli_version()
    system = platform.system()

    print(f"Downloading Codex CLI version: {version}")

    # Build install command based on platform
    if system == "Windows":
        # Use PowerShell installer on Windows
        install_cmd = [
            "powershell",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            f"npm install -g @openai/codex@{version}" if version != "latest" else "npm install -g @openai/codex",
        ]
    else:
        # Use npm on Unix-like systems
        install_cmd = [
            "npm",
            "install",
            "-g",
            f"@openai/codex@{version}" if version != "latest" else "@openai/codex",
        ]

    try:
        subprocess.run(
            install_cmd,
            check=True,
            capture_output=True,
        )
    except subprocess.CalledProcessError as e:
        print(f"Error downloading CLI: {e}", file=sys.stderr)
        print(f"stdout: {e.stdout.decode()}", file=sys.stderr)
        print(f"stderr: {e.stderr.decode()}", file=sys.stderr)
        sys.exit(1)


def copy_cli_to_bundle() -> None:
    """Copy the installed CLI to the package _bundled directory."""
    # Find project root (parent of scripts directory)
    script_dir = Path(__file__).parent
    project_root = script_dir.parent
    bundle_dir = project_root / "src" / "agent_sdk" / "_bundled"

    # Ensure bundle directory exists
    bundle_dir.mkdir(parents=True, exist_ok=True)

    # Find installed CLI
    cli_path = find_installed_cli()
    if not cli_path:
        print("Error: Could not find installed Codex CLI binary", file=sys.stderr)
        sys.exit(1)

    print(f"Found CLI at: {cli_path}")

    # Determine target filename based on platform
    system = platform.system()
    target_name = "codex.exe" if system == "Windows" else "codex"
    target_path = bundle_dir / target_name

    # Copy the binary
    print(f"Copying CLI to: {target_path}")
    shutil.copy2(cli_path, target_path)

    # Make it executable (Unix-like systems)
    if system != "Windows":
        target_path.chmod(0o755)

    print(f"Successfully bundled CLI binary: {target_path}")

    # Print size info
    size_mb = target_path.stat().st_size / (1024 * 1024)
    print(f"Binary size: {size_mb:.2f} MB")


def main() -> None:
    """Main entry point."""
    print("=" * 60)
    print("Codex CLI Download Script")
    print("=" * 60)

    # Download CLI
    download_cli()

    # Copy to bundle directory
    copy_cli_to_bundle()

    print("=" * 60)
    print("CLI download and bundling complete!")
    print("=" * 60)


if __name__ == "__main__":
    main()
