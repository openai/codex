#!/usr/bin/env python3

"""
Utility script to verify (and optionally fix) the Table of Contents in a
Markdown file. By default, it checks that the ToC between `<!-- Begin ToC -->`
and `<!-- End ToC -->` matches the headings in the file. With --fix, it
rewrites the file to update the ToC.
"""

import argparse
import sys
import re
import difflib
from pathlib import Path
from typing import List

# Markers for the Table of Contents section
BEGIN_TOC: str = "<!-- Begin ToC -->"
END_TOC: str = "<!-- End ToC -->"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check and optionally fix the README.md Table of Contents."
    )
    parser.add_argument(
        "file", nargs="?", default="README.md", help="Markdown file to process"
    )
    parser.add_argument(
        "--fix", action="store_true", help="Rewrite file with updated ToC"
    )
    args = parser.parse_args()
    path = Path(args.file)
    return check_or_fix(path, args.fix)


def generate_toc_lines(content: str) -> List[str]:
    """
    Generate markdown list lines for headings (## to ######) in content.
    """
    lines = content.splitlines()
    headings = []
    in_code = False
    for line in lines:
        if line.strip().startswith("```"):
            in_code = not in_code
            continue
        if in_code:
            continue
        m = re.match(r"^(#{2,6})\s+(.*)$", line)
        if not m:
            continue
        level = len(m.group(1))
        text = m.group(2).strip()
        headings.append((level, text))

    toc = []
    for level, text in headings:
        indent = "  " * (level - 2)
        slug = text.lower()
        # normalize spaces and dashes
        slug = slug.replace("\u00a0", " ")
        slug = slug.replace("\u2011", "-").replace("\u2013", "-").replace("\u2014", "-")
        # drop other punctuation
        slug = re.sub(r"[^0-9a-z\s-]", "", slug)
        slug = slug.strip().replace(" ", "-")
        toc.append(f"{indent}- [{text}](#{slug})")
    return toc


def check_or_fix(readme_path: Path, fix: bool) -> int:
    if not readme_path.is_file():
        print(f"Error: file not found: {readme_path}", file=sys.stderr)
        return 1
    content = readme_path.read_text(encoding="utf-8")
    lines = content.splitlines()
    # locate ToC markers
    try:
        begin_idx = next(i for i, l in enumerate(lines) if l.strip() == BEGIN_TOC)
        end_idx = next(i for i, l in enumerate(lines) if l.strip() == END_TOC)
    except StopIteration:
        # No ToC markers found; treat as a no-op so repos without a ToC don't fail CI
        print(
            f"Note: Skipping ToC check; no markers found in {readme_path}.",
        )
        return 0
    # extract current ToC list items
    current_block = lines[begin_idx + 1 : end_idx]
    current = [l for l in current_block if l.lstrip().startswith("- [")]
    # generate expected ToC from content without current ToC
    toc_content = lines[:begin_idx] + lines[end_idx+1:]
    expected = generate_toc_lines("\n".join(toc_content))
    if current == expected:
        return 0
    if not fix:
        print(
            "ERROR: README ToC is out of date. Diff between existing and generated ToC:"
        )
        # Show full unified diff of current vs expected
        diff = difflib.unified_diff(
            current,
            expected,
            fromfile="existing ToC",
            tofile="generated ToC",
            lineterm="",
        )
        for line in diff:
            print(line)
        return 1
    # rebuild file with updated ToC
    prefix = lines[: begin_idx + 1]
    suffix = lines[end_idx+1:]
    new_lines = prefix + [""] + expected + [""] + suffix
    readme_path.write_text("\n".join(new_lines) + "\n", encoding="utf-8")
    print(f"Updated ToC in {readme_path}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
