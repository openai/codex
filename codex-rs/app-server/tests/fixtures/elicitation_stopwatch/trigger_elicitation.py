#!/usr/bin/env python3

import argparse
import os
import sys
import time
from pathlib import Path

REQUESTED_FILENAME = "elicitation_requested"
RELEASE_FILENAME = "elicitation_release"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--state-dir",
        required=True,
        type=Path,
        help="Directory shared with the test orchestrator.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    state_dir: Path = args.state_dir
    state_dir.mkdir(parents=True, exist_ok=True)

    requested = state_dir / REQUESTED_FILENAME
    release = state_dir / RELEASE_FILENAME

    requested.write_text(f"pid={os.getpid()}\n", encoding="utf-8")
    print("waited for a user approval", file=sys.stderr, flush=True)

    while not release.exists():
        time.sleep(0.05)

    print("approval received", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
