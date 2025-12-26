#!/usr/bin/env python3

import shutil
import subprocess
import sys


def main() -> int:
    if len(sys.argv) not in (2, 3):
        print("Usage: play_sound.py <SOUND_FILE> [DURATION_SECONDS]", file=sys.stderr)
        return 2

    sound = sys.argv[1]
    duration = None
    if len(sys.argv) == 3:
        try:
            duration = float(sys.argv[2])
        except ValueError:
            print("DURATION_SECONDS must be a number", file=sys.stderr)
            return 2
        if duration <= 0:
            print("DURATION_SECONDS must be > 0", file=sys.stderr)
            return 2

    if shutil.which("afplay") is None:
        print("afplay not found (macOS only sample)", file=sys.stderr)
        return 2

    cmd = ["afplay"]
    if duration is not None:
        cmd.extend(["-t", str(duration)])
    cmd.append(sound)
    subprocess.run(cmd, check=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
