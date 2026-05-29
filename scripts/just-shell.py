#!/usr/bin/env python3
"""Cross-platform shell launcher for `just` recipes.

This keeps recipe bodies as normal shell snippets while giving the justfile one
portable placeholder, `{args}`, for forwarding variadic recipe arguments.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys


ARGS_TOKEN = "{args}"
POWERSHELL_ARGS = "@($args | Select-Object -Skip 1)"
SH_ARGS = '"$@"'


def main() -> int:
    if len(sys.argv) < 2:
        print("just shell adapter expected a recipe command.", file=sys.stderr)
        return 1

    command = sys.argv[1]
    recipe_name = sys.argv[2] if len(sys.argv) > 2 else ""
    recipe_args = sys.argv[3:]

    if os.name == "nt":
        return run_powershell(command, recipe_name, recipe_args)
    else:
        return run_sh(command, recipe_name, recipe_args)


def run_sh(command: str, recipe_name: str, recipe_args: list[str]) -> int:
    command = command.replace(ARGS_TOKEN, SH_ARGS)
    return subprocess.run(
        ["sh", "-cu", command, recipe_name, *recipe_args],
        check=False,
    ).returncode


def run_powershell(command: str, recipe_name: str, recipe_args: list[str]) -> int:
    pwsh = shutil.which("pwsh.exe") or shutil.which("pwsh")
    if pwsh is None:
        print(
            "PowerShell ('pwsh') is required for Windows just recipes. "
            "Run 'just install' to install it.",
            file=sys.stderr,
        )
        return 1

    command = command.replace(ARGS_TOKEN, POWERSHELL_ARGS)
    return subprocess.run(
        [
            pwsh,
            "-NoLogo",
            "-NoProfile",
            "-CommandWithArgs",
            command,
            recipe_name,
            *recipe_args,
        ],
        check=False,
    ).returncode


if __name__ == "__main__":
    raise SystemExit(main())
