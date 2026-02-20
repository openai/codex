#!/usr/bin/env python3

import argparse
import sys
import time
from pathlib import Path

REQUESTED_FILENAME = "elicitation_requested"
RELEASE_FILENAME = "elicitation_release"


def requested_path(state_dir: Path) -> Path:
    return state_dir / REQUESTED_FILENAME


def release_path(state_dir: Path) -> Path:
    return state_dir / RELEASE_FILENAME


def cmd_wait_for_request(state_dir: Path, timeout_seconds: float) -> int:
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        if requested_path(state_dir).exists():
            return 0
        time.sleep(0.05)

    print(
        f"timed out waiting for {requested_path(state_dir)}",
        file=sys.stderr,
    )
    return 2


def cmd_release(state_dir: Path) -> int:
    release_path(state_dir).write_text("approved\n", encoding="utf-8")
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--state-dir",
        required=True,
        type=Path,
        help="Directory shared with the elicitation trigger script.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    wait_parser = subparsers.add_parser("wait-for-request")
    wait_parser.add_argument(
        "--timeout-seconds",
        type=float,
        default=5.0,
    )

    subparsers.add_parser("release")

    return parser.parse_args()


def main() -> int:
    args = parse_args()
    state_dir: Path = args.state_dir
    state_dir.mkdir(parents=True, exist_ok=True)

    if args.command == "wait-for-request":
        return cmd_wait_for_request(state_dir, args.timeout_seconds)

    if args.command == "release":
        return cmd_release(state_dir)

    print(f"unsupported command: {args.command}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
