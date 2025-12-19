#!/usr/bin/env python3
"""List plan summaries by reading frontmatter only."""

from __future__ import annotations

import argparse
import json

from plan_utils import get_plans_dir, parse_frontmatter


def main() -> int:
    parser = argparse.ArgumentParser(description="List plan summaries from $CODEX_HOME/plans.")
    parser.add_argument("--query", help="Case-insensitive substring to filter name/description.")
    parser.add_argument("--json", action="store_true", help="Emit JSON output.")
    args = parser.parse_args()

    plans_dir = get_plans_dir()
    if not plans_dir.exists():
        raise SystemExit(f"Plans directory not found: {plans_dir}")

    query = args.query.lower() if args.query else None
    items = []
    for path in sorted(plans_dir.glob("*.md")):
        try:
            data = parse_frontmatter(path)
        except ValueError:
            continue
        name = data.get("name")
        description = data.get("description")
        if not name or not description:
            continue
        if query:
            haystack = f"{name} {description}".lower()
            if query not in haystack:
                continue
        items.append({"name": name, "description": description, "path": str(path)})

    if args.json:
        print(json.dumps(items))
    else:
        for item in items:
            print(f"{item['name']}\t{item['description']}\t{item['path']}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
