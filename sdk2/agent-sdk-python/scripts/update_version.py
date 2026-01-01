#!/usr/bin/env python3
"""Update version in pyproject.toml and __init__.py files."""

import re
import sys
from pathlib import Path


def update_version(new_version: str) -> None:
    """Update version in project files."""
    # Update pyproject.toml
    pyproject_path = Path("pyproject.toml")
    content = pyproject_path.read_text()

    # Only update the version field in [project] section
    content = re.sub(
        r'^version = "[^"]*"',
        f'version = "{new_version}"',
        content,
        count=1,
        flags=re.MULTILINE,
    )

    pyproject_path.write_text(content)
    print(f"Updated pyproject.toml to version {new_version}")

    # Update _version.py
    version_path = Path("src/agent_sdk/_version.py")
    content = version_path.read_text()

    # Only update __version__ assignment
    content = re.sub(
        r'^__version__ = "[^"]*"',
        f'__version__ = "{new_version}"',
        content,
        count=1,
        flags=re.MULTILINE,
    )

    version_path.write_text(content)
    print(f"Updated _version.py to version {new_version}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python scripts/update_version.py <version>")
        sys.exit(1)

    update_version(sys.argv[1])
