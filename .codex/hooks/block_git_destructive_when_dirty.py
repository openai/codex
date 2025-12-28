#!/usr/bin/env python3

import json
import re
import subprocess
import sys
from typing import Any, Optional


DESTRUCTIVE_GIT_PATTERNS = [
    # Discards working tree changes
    r"\bgit\s+reset\s+--hard\b",
    r"\bgit\s+checkout\s+--\b",
    r"\bgit\s+restore\b",
    r"\bgit\s+clean\s+-[^\n]*\bf\b",
    # Can discard if used incorrectly / without intent (keep it conservative)
    r"\bgit\s+switch\b",
]


def _extract_command_text(payload: dict[str, Any]) -> str:
    tool_input = payload.get("tool_input")
    if not isinstance(tool_input, dict):
        return ""

    tool_type = tool_input.get("type")
    if tool_type == "function":
        args = tool_input.get("arguments")
        if not isinstance(args, str) or not args:
            return ""
        try:
            obj = json.loads(args)
        except json.JSONDecodeError:
            return args

        # exec_command tool
        if isinstance(obj, dict) and isinstance(obj.get("cmd"), str):
            return obj["cmd"]

        # shell tool
        if isinstance(obj, dict) and isinstance(obj.get("command"), list):
            if all(isinstance(x, str) for x in obj["command"]):
                return " ".join(obj["command"])

        # shell_command tool
        if isinstance(obj, dict) and isinstance(obj.get("command"), str):
            return obj["command"]

        return args

    if tool_type == "local_shell":
        cmd = tool_input.get("command")
        if isinstance(cmd, list) and all(isinstance(x, str) for x in cmd):
            return " ".join(cmd)

    return ""


def _is_git_dirty() -> Optional[bool]:
    try:
        # `--porcelain=v1` is stable and easy to parse.
        out = subprocess.run(
            ["git", "status", "--porcelain=v1"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        ).stdout
    except OSError:
        return None

    # If not a git repo, git returns non-zero with empty stdout, treat as "unknown".
    if out is None:
        return None
    return out.strip() != ""


def main() -> int:
    payload = json.load(sys.stdin)
    cmd_text = _extract_command_text(payload)
    if not cmd_text:
        return 0

    if not any(re.search(p, cmd_text) for p in DESTRUCTIVE_GIT_PATTERNS):
        return 0

    dirty = _is_git_dirty()
    if dirty is True or dirty is None:
        print(
            "BLOCKED: working tree has uncommitted changes (or state unknown).",
            file=sys.stderr,
        )
        print(f"Attempted: {cmd_text}", file=sys.stderr)
        print(
            "Commit/stash your changes, or run the command manually if you really intend it.",
            file=sys.stderr,
        )
        return 2

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
