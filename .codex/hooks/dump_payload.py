#!/usr/bin/env python3

import json
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 2:
        print("Usage: dump_payload.py <OUT_RELATIVE_PATH>", file=sys.stderr)
        return 2

    out_rel = sys.argv[1]
    payload = json.load(sys.stdin)

    cwd = payload.get("cwd")
    if not isinstance(cwd, str) or not cwd:
        print("hook payload missing required string field: cwd", file=sys.stderr)
        return 2

    out_path = Path(cwd) / out_rel
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with out_path.open("a", encoding="utf-8") as f:
        json.dump(payload, f, ensure_ascii=False)
        f.write("\n")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

