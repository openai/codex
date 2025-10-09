#!/usr/bin/env python3
"""Extract a concise Markdown summary from the RTZR docs landing page."""

from __future__ import annotations

import argparse
import re
from pathlib import Path
from typing import List

from bs4 import BeautifulSoup
from bs4.element import NavigableString, Tag


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Organize the RTZR docs landing page into Markdown.")
    parser.add_argument(
        "input_html",
        nargs="?",
        default="rtzr_docs/developers_rtzr_ai_docs.html",
        help="Path to the downloaded RTZR docs HTML file.",
    )
    parser.add_argument(
        "-o",
        "--output",
        default="rtzr_docs/rtzr_docs_summary.md",
        help="Destination path for the generated Markdown summary.",
    )
    return parser.parse_args()


def normalize_spacing(text: str) -> str:
    """Trim extraneous whitespace around punctuation for natural reading."""
    text = re.sub(r"\s+([,.;:!?])", r"\1", text)
    text = re.sub(r"([\(\[\{])\s+", r"\1", text)
    text = re.sub(r"\s+([\)\]\}])", r"\1", text)
    text = re.sub(
        r"([가-힣])\s+(를|을|은|는|이|가|과|와|도|만|까지|부터|으로|로|에|에서|에게|께|한테|처럼|같이|보다)(?=\s|[,.!?;:]|$)",
        r"\1\2",
        text,
    )
    text = re.sub(r"([A-Za-z0-9])\s+의(?=\s|[,.!?;:]|$)", r"\1의", text)
    text = re.sub(r"\s{2,}", " ", text)
    return text.strip()


def clean_text(node: Tag | NavigableString) -> str:
    """Collapse whitespace and return the textual content for a node."""
    text = " ".join(str(node).split())
    return normalize_spacing(text)


def element_text(tag: Tag) -> str:
    """Return normalized text for a tag, preserving inline code tokens."""
    text = tag.get_text(" ", strip=True)
    return normalize_spacing(" ".join(text.split()))


def emit_header(tag: Tag) -> List[str]:
    level = int(tag.name[1])
    title = element_text(tag)
    if not title:
        return []
    return ["#" * level + " " + title, ""]


def emit_paragraph(tag: Tag) -> List[str]:
    text = element_text(tag)
    if not text:
        return []
    return [text, ""]


def emit_code_block(tag: Tag) -> List[str]:
    code_tag = tag.find("code")
    language = ""
    if code_tag and isinstance(code_tag, Tag):
        for cls in code_tag.get("class", []):
            if cls.startswith("language-"):
                language = cls.split("-", 1)[1]
                break
    source_tag = code_tag if isinstance(code_tag, Tag) else tag
    code_text = source_tag.get_text("\n", strip=True)
    if not code_text:
        return []
    fence = f"```{language}" if language else "```"
    return [fence, code_text, "```", ""]


def emit_list(tag: Tag, indent: int = 0) -> List[str]:
    lines: List[str] = []
    ordered = tag.name == "ol"
    index = 1
    for item in tag.find_all("li", recursive=False):
        text_fragments: List[str] = []
        nested_lists: List[Tag] = []
        for child in item.contents:
            if isinstance(child, NavigableString):
                text_fragments.append(clean_text(child))
            elif isinstance(child, Tag):
                if child.name in {"ul", "ol"}:
                    nested_lists.append(child)
                else:
                    text_fragments.append(element_text(child))
        text = normalize_spacing(" ".join(fragment for fragment in text_fragments if fragment))
        prefix = f"{'  ' * indent}{index}. " if ordered else f"{'  ' * indent}- "
        if text:
            lines.append(prefix + text)
        else:
            lines.append(prefix.rstrip())
        for nested in nested_lists:
            lines.extend(emit_list(nested, indent + 1))
        if ordered:
            index += 1
    if lines:
        lines.append("")
    return lines


def consume_node(tag: Tag) -> List[str]:
    if tag.name == "header":
        heading = tag.find(["h1", "h2", "h3", "h4"])
        if heading and isinstance(heading, Tag):
            return emit_header(heading)
        return []
    if tag.name in {"h1", "h2", "h3", "h4"}:
        return emit_header(tag)
    if tag.name == "p":
        return emit_paragraph(tag)
    if tag.name in {"ul", "ol"}:
        return emit_list(tag)
    if tag.name == "pre":
        return emit_code_block(tag)
    if tag.name == "blockquote":
        quote_lines: List[str] = []
        for child in tag.children:
            if isinstance(child, Tag):
                quote_lines.extend(consume_node(child))
        if quote_lines:
            body = ["> " + line if line else ">" for line in quote_lines]
            body.append("")
            return body
    # Recurse into other tags to capture nested content.
    lines: List[str] = []
    for child in tag.children:
        if isinstance(child, Tag):
            lines.extend(consume_node(child))
    return lines


def build_summary(content: Tag) -> str:
    lines: List[str] = []
    for child in content.children:
        if isinstance(child, Tag):
            if child.name == "a" and child.get("href", "").startswith("/docs/"):
                # Skip navigation links rendered at the bottom.
                continue
            lines.extend(consume_node(child))
    # Trim trailing blank lines.
    while lines and not lines[-1].strip():
        lines.pop()
    return "\n".join(lines) + "\n"


def main() -> None:
    args = parse_args()
    input_path = Path(args.input_html)
    output_path = Path(args.output)

    if not input_path.exists():
        raise SystemExit(f"Input file not found: {input_path}")

    html = input_path.read_text(encoding="utf-8")
    soup = BeautifulSoup(html, "html.parser")
    content = soup.select_one('.theme-doc-markdown')
    if content is None:
        raise SystemExit("Unable to locate the main markdown container in the HTML document.")

    summary = build_summary(content)
    output_path.write_text(summary, encoding="utf-8")
    print(f"Wrote summary to {output_path}")


if __name__ == "__main__":
    main()
