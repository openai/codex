#!/usr/bin/env python3

import json
import shutil
import subprocess
import sys


def main() -> int:
    payload = json.load(sys.stdin)
    if payload.get("type") != "tool.exec.end":
        return 0

    if shutil.which("afplay") is None:
        print("afplay not found (macOS only sample)", file=sys.stderr)
        return 2

    exit_code = payload.get("exit_code")
    if isinstance(exit_code, int) and exit_code != 0:
        sound = "/System/Library/Sounds/Basso.aiff"
    else:
        sound = "/System/Library/Sounds/Pop.aiff"

    subprocess.run(["afplay", sound], check=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

