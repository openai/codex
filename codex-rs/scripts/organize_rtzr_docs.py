#!/usr/bin/env python3
"""Summarize the downloaded RTZR developers documentation into a Markdown outline."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path
from typing import Dict, List, Tuple


@dataclass
class DocumentSummary:
    relative_path: str
    title: str
    headings: List[Tuple[int, str]]


class SimpleDocParser(HTMLParser):
    """Minimal HTML parser that collects the title and heading hierarchy."""

    def __init__(self) -> None:
        super().__init__()
        self._in_title = False
        self._title_parts: List[str] = []
        self._heading_tag: str | None = None
        self._heading_depth = 0
        self._heading_buffer: List[str] = []
        self.headings: List[Tuple[int, str]] = []
        self._article_depth = 0

    def handle_starttag(self, tag: str, attrs) -> None:  # type: ignore[override]
        tag = tag.lower()
        if tag == "article":
            self._article_depth += 1
        if tag == "title":
            self._in_title = True
            return
        if tag in {"h1", "h2", "h3"} and self._heading_tag is None and self._article_depth > 0:
            self._heading_tag = tag
            self._heading_depth = 0
            self._heading_buffer = []
            return
        if self._heading_tag is not None:
            self._heading_depth += 1

    def handle_endtag(self, tag: str) -> None:  # type: ignore[override]
        tag = tag.lower()
        if tag == "title":
            self._in_title = False
            return
        if tag == "article" and self._article_depth > 0:
            self._article_depth -= 1
            return
        if self._heading_tag is None:
            return
        if tag == self._heading_tag and self._heading_depth == 0:
            text = "".join(self._heading_buffer).strip()
            if text:
                cleaned = " ".join(text.split())
                level = int(self._heading_tag[1])
                self.headings.append((level, cleaned))
            self._heading_tag = None
            self._heading_buffer = []
            return
        if tag == self._heading_tag and self._heading_depth > 0:
            self._heading_depth -= 1
            return
        if self._heading_depth > 0:
            self._heading_depth -= 1

    def handle_data(self, data: str) -> None:  # type: ignore[override]
        if self._in_title:
            self._title_parts.append(data)
        if self._heading_tag is not None:
            self._heading_buffer.append(data)

    def get_title(self) -> str:
        raw = "".join(self._title_parts).strip()
        return " ".join(raw.split())


def parse_html(path: Path) -> DocumentSummary:
    parser = SimpleDocParser()
    try:
        parser.feed(path.read_text(encoding="utf-8"))
    except UnicodeDecodeError:
        parser.feed(path.read_text(encoding="utf-8", errors="ignore"))
    parser.close()
    title = parser.get_title() or "Untitled"
    rel_path = path.as_posix()
    return DocumentSummary(relative_path=rel_path, title=title, headings=parser.headings)


def gather_documents(doc_root: Path) -> List[DocumentSummary]:
    summaries: List[DocumentSummary] = []
    for path in sorted(doc_root.rglob("*.html")):
        if any(part in {"assets", "dassets", "_next", "static"} for part in path.parts):
            continue
        relative = path.relative_to(doc_root)
        summary = parse_html(path)
        summary.relative_path = relative.as_posix()
        summaries.append(summary)
    return summaries


def build_markdown(summaries: List[DocumentSummary], max_headings: int) -> str:
    grouped: Dict[str, List[DocumentSummary]] = {}
    for summary in summaries:
        parts = summary.relative_path.split("/")
        group = "root" if len(parts) == 1 else parts[0]
        grouped.setdefault(group, []).append(summary)

    lines: List[str] = ["# RTZR Developers Docs Summary", ""]
    for group in sorted(grouped.keys(), key=lambda key: (key != "root", key)):
        lines.append(f"## {group}")
        lines.append("")
        for summary in sorted(grouped[group], key=lambda item: item.relative_path):
            lines.append(f"- `{summary.relative_path}` — {summary.title}")
            if summary.headings:
                for level, text in summary.headings[:max_headings]:
                    indent = "    " * level
                    lines.append(f"{indent}- H{level}: {text}")
            else:
                lines.append("    - No headings found.")
            lines.append("")
    return "\n".join(lines).rstrip("\n") + "\n"


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Create a Markdown outline for RTZR docs.")
    parser.add_argument(
        "doc_root",
        nargs="?",
        default="rtzr-docs/developers.rtzr.ai/docs",
        help="Path to the downloaded docs root (default: %(default)s)",
    )
    parser.add_argument(
        "-o",
        "--output",
        default="rtzr-docs-summary.md",
        help="Where to write the Markdown summary (default: %(default)s)",
    )
    parser.add_argument(
        "--max-headings",
        type=int,
        default=15,
        help="Maximum number of headings to include per document (default: %(default)s)",
    )

    args = parser.parse_args(argv)
    doc_root = Path(args.doc_root).expanduser().resolve()
    if not doc_root.exists():
        print(f"Document root not found: {doc_root}", file=sys.stderr)
        return 1
    summaries = gather_documents(doc_root)
    if not summaries:
        print(f"No HTML files found under {doc_root}", file=sys.stderr)
        return 1

    markdown = build_markdown(summaries, args.max_headings)
    output_path = Path(args.output).expanduser()
    output_path.write_text(markdown, encoding="utf-8")
    print(f"Wrote summary to {output_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
