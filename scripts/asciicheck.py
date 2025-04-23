#!/usr/bin/env python3

import argparse
import sys
from pathlib import Path

"""
Utility script that takes a list of files and returns non-zero if any of them
contain non-ASCII characters other than those in the allowed list.

If --fix is used, it will attempt to replace non-ASCII characters with ASCII
equivalents.

The motivation behind this script is that characters like U+00A0 (non-breaking
space) can cause regexes not to match and can result in surprising anchor
values for headings when GitHub renders Markdown as HTML.
"""


"""
When --fix is used, perform the following substitutions.
"""
substitutions: dict[int, str] = {
    0x00A0: " ",  # non-breaking space
    0x2011: "-",  # non-breaking hyphen
    0x2013: "-",  # en dash
    0x2014: "-",  # em dash
    0x2018: "'",  # left single quote
    0x2019: "'",  # right single quote
    0x201C: '"',  # left double quote
    0x201D: '"',  # right double quote
    0x2026: "...",  # ellipsis
    0x202F: " ",  # narrow non-breaking space
}

"""
Unicode codepoints that are allowed in addition to ASCII.
Be conservative with this list.

Note that it is always an option to use the hex HTML representation
instead of the character itself so the source code is ASCII-only.
For example, U+2728 (sparkles) can be written as `&#x2728;`.
"""
allowed_unicode_codepoints = {
    0x2728,  # sparkles
}


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check for non-ASCII characters in files."
    )
    parser.add_argument(
        "--fix",
        action="store_true",
        help="Rewrite files, replacing non-ASCII characters with ASCII equivalents, where possible.",
    )
    parser.add_argument(
        "files",
        nargs="+",
        help="Files to check for non-ASCII characters.",
    )
    args = parser.parse_args()

    has_errors = False
    for filename in args.files:
        path = Path(filename)
        has_errors |= lint_utf8_ascii(path, fix=args.fix)
    return 1 if has_errors else 0


def lint_utf8_ascii(filename: Path, fix: bool) -> bool:
    """Returns True if an error was printed."""
    try:
        with open(filename, "rb") as f:
            raw = f.read()
        text = raw.decode("utf-8")
    except UnicodeDecodeError as e:
        print("UTF-8 decoding error:")
        print(f"  byte offset: {e.start}")
        print(f"  reason: {e.reason}")
        # Attempt to find line/column
        partial = raw[: e.start]
        line = partial.count(b"\n") + 1
        col = e.start - (partial.rfind(b"\n") if b"\n" in partial else -1)
        print(f"  location: line {line}, column {col}")
        return True

    errors = []
    for lineno, line in enumerate(text.splitlines(keepends=True), 1):
        for colno, char in enumerate(line, 1):
            codepoint = ord(char)
            if char == "\n":
                continue
            if (
                not (0x20 <= codepoint <= 0x7E)
                and codepoint not in allowed_unicode_codepoints
            ):
                errors.append((lineno, colno, char, codepoint))

    if errors:
        for lineno, colno, char, codepoint in errors:
            safe_char = repr(char)[1:-1]  # nicely escape things like \u202f
            print(
                f"Invalid character at line {lineno}, column {colno}: U+{codepoint:04X} ({safe_char})"
            )

    if errors and fix:
        print(f"Attempting to fix {filename}...")
        num_replacements = 0
        new_contents = ""
        for char in text:
            codepoint = ord(char)
            if codepoint in substitutions:
                num_replacements += 1
                new_contents += substitutions[codepoint]
            else:
                new_contents += char
        with open(filename, "w", encoding="utf-8") as f:
            f.write(new_contents)
        print(f"Fixed {num_replacements} of {len(errors)} errors in {filename}.")

    return bool(errors)


if __name__ == "__main__":
    sys.exit(main())
