#!/usr/bin/env python3
"""Generate local .eml drafts from outreach templates (no sending)."""
from __future__ import annotations

from pathlib import Path

BASE = Path(__file__).resolve().parents[1]
TEMPLATE_PATH = BASE / "templates" / "outreach_messages.md"
OUTPUT_DIR = BASE / "packet" / "drafts"


def parse_sections(text: str) -> list[tuple[str, str]]:
    sections: list[tuple[str, str]] = []
    current_title: str | None = None
    buffer: list[str] = []

    for line in text.splitlines():
        if line.startswith("## "):
            if current_title:
                sections.append((current_title, "\n".join(buffer).strip()))
                buffer = []
            current_title = line.replace("## ", "").strip()
        elif current_title:
            buffer.append(line)

    if current_title:
        sections.append((current_title, "\n".join(buffer).strip()))

    return sections


def build_eml(subject: str, body: str) -> str:
    headers = [
        "From: [YOUR NAME] <your@email>",
        "To: [RECIPIENT EMAIL]",
        f"Subject: {subject}",
        "MIME-Version: 1.0",
        "Content-Type: text/plain; charset=utf-8",
    ]
    return "\n".join(headers) + "\n\n" + body + "\n"


def main() -> int:
    if not TEMPLATE_PATH.exists():
        print(f"Template not found: {TEMPLATE_PATH}")
        return 1

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    content = TEMPLATE_PATH.read_text(encoding="utf-8")
    sections = parse_sections(content)

    for idx, (title, body) in enumerate(sections, start=1):
        eml = build_eml(title, body)
        output_path = OUTPUT_DIR / f"draft_{idx:02d}.eml"
        output_path.write_text(eml, encoding="utf-8")

    print(f"Generated {len(sections)} drafts in {OUTPUT_DIR}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
