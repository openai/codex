#!/usr/bin/env python3
"""Post-install configuration for the Codex devcontainer."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def ensure_history_files() -> None:
    command_history_dir = Path("/commandhistory")
    command_history_dir.mkdir(parents=True, exist_ok=True)

    for filename in (".bash_history", ".zsh_history"):
        (command_history_dir / filename).touch(exist_ok=True)


def fix_directory_ownership() -> None:
    uid = os.getuid()
    gid = os.getgid()

    paths = [
        Path.home() / ".codex",
        Path.home() / ".config" / "gh",
        Path.home() / ".cargo",
        Path.home() / ".rustup",
        Path("/commandhistory"),
    ]

    for path in paths:
        if not path.exists():
            continue

        stat_info = path.stat()
        if stat_info.st_uid == uid and stat_info.st_gid == gid:
            continue

        try:
            subprocess.run(
                ["sudo", "chown", "-R", f"{uid}:{gid}", str(path)],
                check=True,
                capture_output=True,
                text=True,
            )
            print(f"[post_install] fixed ownership: {path}", file=sys.stderr)
        except subprocess.CalledProcessError as err:
            print(
                f"[post_install] warning: could not fix ownership of {path}: {err.stderr.strip()}",
                file=sys.stderr,
            )


def setup_git_config() -> None:
    home = Path.home()
    host_gitconfig = home / ".gitconfig"
    local_gitconfig = home / ".gitconfig.local"
    gitignore_global = home / ".gitignore_global"

    gitignore_global.write_text(
        """# Codex
.codex/

# Rust
/target/

# Node
node_modules/

# Python
__pycache__/
*.pyc

# Editors
.vscode/
.idea/

# macOS
.DS_Store
""",
        encoding="utf-8",
    )

    include_line = (
        f"[include]\n    path = {host_gitconfig}\n\n" if host_gitconfig.exists() else ""
    )

    local_gitconfig.write_text(
        f"""# Container-local git configuration
{include_line}[core]
    excludesfile = {gitignore_global}

[merge]
    conflictstyle = diff3

[diff]
    colorMoved = default
""",
        encoding="utf-8",
    )


def main() -> None:
    print("[post_install] configuring devcontainer...", file=sys.stderr)
    ensure_history_files()
    fix_directory_ownership()
    setup_git_config()
    print("[post_install] complete", file=sys.stderr)


if __name__ == "__main__":
    main()
