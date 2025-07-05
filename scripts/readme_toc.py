#!/usr/bin/env python3
"""
Verify and optionally fix the Table of Contents in a Markdown file.
By default, it checks that the ToC between markers matches headings.
With --fix, it rewrites the file to update the ToC.
"""
import argparse
import re
import sys
from difflib import unified_diff
from pathlib import Path
from typing import List

# ToC markers
BEGIN_TOC = "<!-- Begin ToC -->"
END_TOC = "<!-- End ToC -->"
# Match headings from ## to ######
HEADING_RE = re.compile(r"^(#{2,6})\s+(.*)$")
# Characters to normalize in slugs
PUNCT_RE = re.compile(r"[^0-9a-z\s-]")


def generate_toc(content: str) -> List[str]:
    """Extract headings and build markdown ToC lines."""
    toc = []
    in_code = False
    for line in content.splitlines():
        # Toggle code block state
        if line.strip().startswith("```"):
            in_code = not in_code
            continue
        if in_code:
            continue
        m = HEADING_RE.match(line)
        if not m:
            continue
        level, title = len(m.group(1)), m.group(2).strip()
        # Build slug: lowercase, normalize dashes, drop punctuation
        slug = title.lower().replace("\u00a0", " ")
        slug = slug.replace("\u2011", "-").replace("\u2013", "-").replace("\u2014", "-")
        slug = PUNCT_RE.sub("", slug).strip().replace(" ", "-")
        indent = "  " * (level - 2)
        toc.append(f"{indent}- [{title}](#{slug})")
    return toc


def process(path: Path, fix: bool) -> int:
    """Check or fix the ToC in the given Markdown file."""
    if not path.is_file():
        print(f"Error: file not found: {path}", file=sys.stderr)
        return 1
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()

    # Locate markers
    try:
        i1 = next(i for i, l in enumerate(lines) if l.strip() == BEGIN_TOC)
        i2 = next(i for i, l in enumerate(lines) if l.strip() == END_TOC)
    except StopIteration:
        print(f"Error: missing ToC markers in {path}", file=sys.stderr)
        return 1

    current = [l for l in lines[i1+1:i2] if l.lstrip().startswith("- [")]
    expected = generate_toc(text)

    if current == expected:
        return 0

    if not fix:
        # Show diff
        print("ERROR: ToC out of date. Diff:")
        for ln in unified_diff(current, expected, fromfile="existing ToC", tofile="generated ToC", lineterm=""):
            print(ln)
        return 1

    # Rewrite file with new ToC
    new = lines[:i1+1] + [""] + expected + [""] + lines[i2:]
    path.write_text("\n".join(new) + "\n", encoding="utf-8")
    print(f"Updated ToC in {path}.")
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("file", nargs="?", type=Path, default=Path("README.md"), help="Markdown file")
    p.add_argument("--fix", action="store_true", help="Rewrite with updated ToC")
    args = p.parse_args()
    sys.exit(process(args.file, args.fix))


if __name__ == "__main__":
    main()
