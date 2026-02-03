#!/usr/bin/env python3
"""Redact sensitive data from text files (email, phone, SSN-like patterns)."""
from __future__ import annotations

import re
import sys
from pathlib import Path

EMAIL_PATTERN = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")
PHONE_PATTERN = re.compile(r"\b(?:\+?1[-.\s]?)?(?:\(?\d{3}\)?[-.\s]?)\d{3}[-.\s]?\d{4}\b")
SSN_PATTERN = re.compile(r"\b\d{3}-\d{2}-\d{4}\b")

REDACTIONS = {
    EMAIL_PATTERN: "[REDACTED_EMAIL]",
    PHONE_PATTERN: "[REDACTED_PHONE]",
    SSN_PATTERN: "[REDACTED_SSN]",
}


def redact_text(text: str) -> str:
    for pattern, replacement in REDACTIONS.items():
        text = pattern.sub(replacement, text)
    return text


def main() -> int:
    if len(sys.argv) < 3:
        print("Usage: redact.py <input_file> <output_file>")
        return 1

    input_path = Path(sys.argv[1]).expanduser()
    output_path = Path(sys.argv[2]).expanduser()

    if not input_path.exists():
        print(f"Input file not found: {input_path}")
        return 1

    content = input_path.read_text(encoding="utf-8")
    redacted = redact_text(content)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(redacted, encoding="utf-8")
    print(f"Redacted file written to {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
