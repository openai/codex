#!/usr/bin/env python3
"""Build a consolidated advocacy packet from templates."""
from __future__ import annotations

import datetime
from pathlib import Path

BASE = Path(__file__).resolve().parents[1]
TEMPLATES = BASE / "templates"
PACKET_DIR = BASE / "packet"

SECTIONS = [
    ("Case Profile", "case_profile.yml"),
    ("Intake Summary", "intake_form.md"),
    ("Chronology", "chronology.csv"),
    ("Evidence Log", "evidence_log.csv"),
    ("Witness Log", "witness_log.csv"),
    ("Outreach Messages", "outreach_messages.md"),
    ("Legal Hold Notice", "legal_hold_notice.md"),
]


def read_or_placeholder(path: Path) -> str:
    if path.exists():
        return path.read_text(encoding="utf-8")
    return "[MISSING FILE]"


def main() -> int:
    PACKET_DIR.mkdir(parents=True, exist_ok=True)
    stamp = datetime.datetime.utcnow().isoformat() + "Z"
    packet_path = PACKET_DIR / "advocacy_packet.md"

    lines = [
        "# Advocacy Packet",
        f"Generated: {stamp}",
        "",
        "This packet is generated locally. Redact before sharing.",
        "",
    ]

    for title, filename in SECTIONS:
        content = read_or_placeholder(TEMPLATES / filename)
        lines.extend([f"## {title}", "", "```", content, "```", ""])

    packet_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"Packet written to {packet_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
