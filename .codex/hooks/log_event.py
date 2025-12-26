#!/usr/bin/env python3

import datetime
import json
from pathlib import Path
import sys


def main() -> int:
    if len(sys.argv) != 2:
        print("Usage: log_event.py <OUT_PATH>", file=sys.stderr)
        return 2

    out_path = Path(sys.argv[1])
    payload = json.load(sys.stdin)

    event_type = payload.get("type", "<unknown>")
    tool_name = payload.get("tool_name")
    call_id = payload.get("call_id")

    now = datetime.datetime.now(datetime.timezone.utc).isoformat()
    summary = f"{now}\t{event_type}"
    if isinstance(tool_name, str):
        summary += f"\ttool={tool_name}"
    if isinstance(call_id, str):
        summary += f"\tcall_id={call_id}"

    out_path.parent.mkdir(parents=True, exist_ok=True)
    with out_path.open("a", encoding="utf-8") as f:
        f.write(summary + "\n")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

