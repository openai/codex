#!/usr/bin/env python3
"""Compare a publishing release with the canary Cargo manifest from stdin.

Usage: check_github_canary.py RELEASE_VERSION
"""

import sys
import tomllib

from releases import should_update_version


def main() -> None:
    try:
        current_version = tomllib.loads(sys.stdin.read())["workspace"]["package"][
            "version"
        ]
    except (tomllib.TOMLDecodeError, KeyError, TypeError):
        current_version = ""
    if not isinstance(current_version, str):
        current_version = ""
    print(str(should_update_version(sys.argv[1], current_version)).lower())


if __name__ == "__main__":
    main()
