#!/usr/bin/env python3
"""Generate a local Drive folder manifest for manual upload."""
from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path

BASE = Path(__file__).resolve().parents[1]
OUTPUT = BASE / "data" / "metadata" / "drive_manifest.json"

FOLDERS = [
    "Advocacy Packet",
    "Evidence (Raw - Local Only)",
    "Evidence (Redacted)",
    "Chronology",
    "Witness Statements",
    "Legal Notices",
]


def main() -> int:
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    manifest = {
        "generated_at": datetime.utcnow().isoformat() + "Z",
        "folders": [{"name": name, "children": []} for name in FOLDERS],
        "note": "Manual upload only. Keep raw evidence offline unless necessary.",
    }
    OUTPUT.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    print(f"Drive manifest written to {OUTPUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
