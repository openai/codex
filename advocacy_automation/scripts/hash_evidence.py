#!/usr/bin/env python3
"""Hash evidence files to create tamper-evident records."""
from __future__ import annotations

import csv
import hashlib
import json
import os
import sys
from datetime import datetime
from pathlib import Path


def sha256_file(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def main() -> int:
    if len(sys.argv) < 2:
        print("Usage: hash_evidence.py <evidence_dir>")
        return 1

    evidence_dir = Path(sys.argv[1]).expanduser().resolve()
    if not evidence_dir.exists():
        print(f"Evidence directory not found: {evidence_dir}")
        return 1

    output_dir = evidence_dir.parent / "metadata"
    output_dir.mkdir(parents=True, exist_ok=True)

    timestamp = datetime.utcnow().isoformat() + "Z"
    records: list[dict[str, str]] = []

    for path in sorted(evidence_dir.rglob("*")):
        if path.is_file():
            records.append(
                {
                    "path": str(path.relative_to(evidence_dir)),
                    "sha256": sha256_file(path),
                    "bytes": str(path.stat().st_size),
                    "hashed_at": timestamp,
                }
            )

    json_path = output_dir / "evidence_hashes.json"
    csv_path = output_dir / "evidence_hashes.csv"

    with json_path.open("w", encoding="utf-8") as handle:
        json.dump({"generated_at": timestamp, "records": records}, handle, indent=2)

    with csv_path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=["path", "sha256", "bytes", "hashed_at"])
        writer.writeheader()
        writer.writerows(records)

    print(f"Wrote {len(records)} hashes to {json_path} and {csv_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
