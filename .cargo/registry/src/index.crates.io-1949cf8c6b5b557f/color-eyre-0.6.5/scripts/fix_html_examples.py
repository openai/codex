#! /usr/bin/env python3.8

from __future__ import annotations

import argparse
from argparse import FileType, ArgumentParser
import enum
import sys

from bs4 import BeautifulSoup, Tag


class LineType(enum.Enum):
    OUTER_DOC = enum.auto()
    INNER_DOC = enum.auto()
    SOURCE = enum.auto()

    @classmethod
    def from_line(cls, line: str) -> (LineType, str):
        if line.startswith("//!"):
            return (cls.OUTER_DOC, line[len("//!") :])
        elif line.startswith("///"):
            return (cls.INNER_DOC, line[len("///") :])
        else:
            return (cls.SOURCE, line)

    def prefix(self) -> str:
        if self == LineType.OUTER_DOC:
            return "//!"
        elif self == LineType.INNER_DOC:
            return "///"
        else:
            return ""


def fix_gnome_html(fh: file) -> str:
    """Tweaks for fixing "Copy as HTML" output from gnome-terminal

    Reads source from a Rust file.
    """

    anything_changed = False
    line_type = LineType.SOURCE

    # Lines of current HTML <pre> chunk
    pre_chunk = []
    # Lines of processed file
    ret = []

    for (line_type, stripped_line), line in map(
        lambda line: (LineType.from_line(line), line), fh.readlines()
    ):
        if line_type == LineType.SOURCE:
            ret.append(line)
        elif stripped_line.lstrip().startswith("<pre"):
            pre_chunk = [stripped_line]
        elif stripped_line.rstrip().endswith("</pre>"):
            pre_chunk.append(stripped_line)
            if any("<font" in line for line in pre_chunk):
                joined_chunk = "".join(pre_chunk)
                fixed_chunk = fix_pre(joined_chunk, prefix=line_type.prefix())
                anything_changed = joined_chunk != fixed_chunk
                ret.append(fixed_chunk)
                pre_chunk = []
            else:
                prefix = line_type.prefix()
                ret.extend(line_type.prefix() + line for line in pre_chunk)
        elif pre_chunk:
            pre_chunk.append(stripped_line)
        else:
            ret.append(line)

    return "".join(ret) if anything_changed else None


def fix_pre(html: str, prefix: str = "") -> str:
    """Fixes an individual <pre> tag from Gnome.

    Optionally prepends a given prefix to each line in the returned output.
    """
    soup = BeautifulSoup(html, "html.parser")

    for pre in soup.find_all("pre"):
        for tag in pre.find_all("font"):
            # <font color=xxx> -> <span style="color: xxx">
            tag.name = "span"
            color = tag.attrs.pop("color")
            tag["style"] = f"color: {color}"

    return "".join(prefix + line for line in str(soup).splitlines(keepends=True))


def main():
    parser = ArgumentParser(
        description="""Convert HTML from Gnome terminal's 'Copy as HTML' feature
        to use modern <span> tags and inline CSS.

        This script is idempotent, i.e. multiple invocations will not change
        the output past the first invocation."""
    )
    parser.add_argument(
        "file",
        nargs="+",
        type=FileType("r+", encoding="utf-8"),
        help="""Rust file to update <pre> blocks in.""",
    )
    args = parser.parse_args()
    for fh in args.file:
        if not fh.name.endswith(".rs"):
            print(
                "This script only fixes Rust source files; you probably didn't mean to include",
                fh.name,
                "so I'll skip processing it.",
            )
        new_content = fix_gnome_html(fh)
        if new_content is not None:
            print("Updated example colored output in", fh.name)
            fh.seek(0)
            fh.write(new_content)
        else:
            print("Nothing to fix in", fh.name)
        fh.close()


if __name__ == "__main__":
    main()
