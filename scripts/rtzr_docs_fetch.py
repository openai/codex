#!/usr/bin/env python3
"""Download RTZR STT docs and summarize them into a single Markdown file."""

from __future__ import annotations

import argparse
import datetime as dt
import re
import sys
from collections import deque
from pathlib import Path
from typing import Iterable
from urllib.parse import urljoin, urlparse, urlunparse, urldefrag

import html2text
import requests
from bs4 import BeautifulSoup


DEFAULT_BASE_URL = "https://developers.rtzr.ai"
DEFAULT_ROOT_PATH = "/docs/"
DEFAULT_OUTPUT = "rtzr_docs/rtzr_docs_summary.md"
LANGUAGE_EXCLUDES = ("/docs/en/", "/docs/ja/")
ASSET_PATH_SNIPPETS = ("/dassets/", "/assets/")
SKIP_EXTENSIONS = (
    ".css",
    ".js",
    ".png",
    ".jpg",
    ".jpeg",
    ".svg",
    ".ico",
    ".json",
    ".pdf",
    ".gif",
    ".mp3",
    ".mp4",
    ".zip",
    ".xml",
    ".rss",
    ".atom",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--root-path", default=DEFAULT_ROOT_PATH)
    parser.add_argument("--output", default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--max-pages",
        type=int,
        default=None,
        help="Optional upper bound on the number of pages to crawl.",
    )
    return parser.parse_args()


def normalize_url(raw_href: str, source_url: str, base_netloc: str, root_path: str) -> str | None:
    absolute = urljoin(source_url, raw_href)
    absolute, _ = urldefrag(absolute)
    parsed = urlparse(absolute)
    if parsed.scheme not in {"http", "https"}:
        return None
    if parsed.netloc != base_netloc:
        return None
    path = parsed.path
    if not path.startswith(root_path.rstrip("/")):
        return None
    if any(path.startswith(prefix.rstrip("/")) for prefix in LANGUAGE_EXCLUDES):
        return None
    if any(path.endswith(ext) for ext in SKIP_EXTENSIONS):
        return None
    if any(snippet in path for snippet in ASSET_PATH_SNIPPETS):
        return None
    if path != root_path and not path.endswith("/"):
        path = f"{path}/"
    normalized = parsed._replace(path=path, params="", query="", fragment="")
    return urlunparse(normalized)


def clean_article(article: BeautifulSoup) -> None:
    for selector in (
        "nav.theme-doc-breadcrumbs",
        "div.tocCollapsible_ETCw",
        "div[class*='tocDesktop']",
    ):
        for node in article.select(selector):
            node.decompose()


def collapse_blank_lines(markdown: str) -> str:
    return re.sub(r"\n{3,}", "\n\n", markdown.strip())


def strip_leading_title(markdown: str, title: str) -> str:
    pattern = rf"^#\s+{re.escape(title)}\s*\n"
    return re.sub(pattern, "", markdown, count=1).lstrip()


def crawl_docs(base_url: str, root_path: str, max_pages: int | None) -> list[dict[str, str]]:
    base_parsed = urlparse(base_url)
    start_url = urljoin(base_url, root_path)
    queue: deque[str] = deque([start_url])
    visited: set[str] = set()
    docs: list[dict[str, str]] = []

    session = requests.Session()
    session.headers.update({"User-Agent": "codex-doc-fetcher/1.0"})

    converter = html2text.HTML2Text()
    converter.ignore_links = False
    converter.ignore_images = True
    converter.body_width = 0

    while queue:
        if max_pages is not None and len(visited) >= max_pages:
            break
        current = queue.popleft()
        if current in visited:
            continue
        visited.add(current)

        try:
            response = session.get(current, timeout=30)
            response.raise_for_status()
        except requests.RequestException as err:
            print(f"[warn] failed to fetch {current}: {err}", file=sys.stderr)
            continue

        response.encoding = "utf-8"
        soup = BeautifulSoup(response.text, "html.parser")
        article = soup.select_one("article")
        if not article:
            continue

        clean_article(article)

        title = article.find("h1")
        if title:
            title_text = title.get_text(strip=True)
        elif soup.title and soup.title.string:
            title_text = soup.title.string.strip()
        else:
            title_text = current
        markdown = converter.handle(str(article))
        markdown = collapse_blank_lines(strip_leading_title(markdown, title_text))

        docs.append({
            "url": current,
            "title": title_text,
            "markdown": markdown,
        })

        for link in soup.select("a[href]"):
            href = link.get("href")
            if not href:
                continue
            normalized = normalize_url(href, current, base_parsed.netloc, root_path)
            if not normalized:
                continue
            if normalized in visited or normalized in queue:
                continue
            queue.append(normalized)

    return docs


def ensure_parent_dir(path: Path) -> None:
    if path.parent.exists():
        return
    path.parent.mkdir(parents=True, exist_ok=True)


def write_summary(docs: Iterable[dict[str, str]], output_path: Path) -> None:
    ensure_parent_dir(output_path)
    timestamp = dt.datetime.now(dt.timezone.utc).astimezone().isoformat(timespec="seconds")

    with output_path.open("w", encoding="utf-8") as handle:
        handle.write("# RTZR STT OpenAPI 문서 요약\n")
        handle.write(f"\n_생성 시각: {timestamp}_\n\n")
        for entry in docs:
            handle.write(f"## {entry['title']}\n")
            handle.write(f"- 출처: {entry['url']}\n\n")
            handle.write(f"{entry['markdown']}\n")
            handle.write("\n---\n\n")


def main() -> int:
    args = parse_args()
    docs = crawl_docs(args.base_url, args.root_path, args.max_pages)
    if not docs:
        print("[error] no documents were collected", file=sys.stderr)
        return 1
    output_path = Path(args.output)
    write_summary(docs, output_path)
    print(f"Collected {len(docs)} pages into {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
