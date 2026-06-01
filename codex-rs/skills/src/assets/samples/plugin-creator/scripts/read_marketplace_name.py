#!/usr/bin/env python3
"""Print the top-level marketplace name from any marketplace.json file."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def default_marketplace_path() -> Path:
    return Path.home() / ".agents" / "plugins" / "marketplace.json"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Print the top-level marketplace name from marketplace.json. Defaults to the personal "
            "marketplace path under the current home directory."
        )
    )
    parser.add_argument(
        "--marketplace-path",
        default=str(default_marketplace_path()),
        help="Path to marketplace.json",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    marketplace_path = Path(args.marketplace_path).expanduser().resolve()
    payload = json.loads(marketplace_path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{marketplace_path} must contain a JSON object.")
    name = payload.get("name")
    if not isinstance(name, str) or not name.strip():
        raise ValueError(f"{marketplace_path} must contain a non-empty string 'name'.")
    print(name.strip())


if __name__ == "__main__":
    try:
        main()
    except Exception as err:  # noqa: BLE001 - CLI should surface a single clear message.
        print(str(err), file=sys.stderr)
        raise SystemExit(1) from err
