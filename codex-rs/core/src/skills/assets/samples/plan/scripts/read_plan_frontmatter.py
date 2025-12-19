#!/usr/bin/env python3
"""Read plan frontmatter without loading the full markdown body."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from plan_utils import parse_frontmatter


def main() -> int:
    parser = argparse.ArgumentParser(description="Read name/description from plan frontmatter.")
    parser.add_argument("plan_path", help="Path to the plan markdown file.")
    parser.add_argument("--json", action="store_true", help="Emit JSON output.")
    args = parser.parse_args()

    path = Path(args.plan_path).expanduser()
    if not path.exists():
        raise SystemExit(f"Plan not found: {path}")

    data = parse_frontmatter(path)
    name = data.get("name")
    description = data.get("description")
    if not name or not description:
        raise SystemExit("Frontmatter must include name and description.")

    payload = {"name": name, "description": description, "path": str(path)}
    if args.json:
        print(json.dumps(payload))
    else:
        print(f"name: {name}")
        print(f"description: {description}")
        print(f"path: {path}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
