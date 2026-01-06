#!/usr/bin/env python3
"""
Fetch GitHub Releases for a repo and map release tags to git commits.

Example:
  ./scripts/map_upstream_releases.py --repo openai/codex --remote upstream --semver-only --format tsv
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import os
import urllib.error
import urllib.request
from dataclasses import asdict, dataclass
from typing import Any, Iterable


API_BASE = "https://api.github.com"
DEFAULT_REPO = "openai/codex"
DEFAULT_REMOTE = "upstream"
DEFAULT_TAG_PREFIX = "rust-v"

# rust-v0.77.0 / rust-v0.77.0-alpha.1 / rust-v0.77.0-beta / rust-v0.77.0-beta.2
SEMVER_TAG_RE = re.compile(
    r"^rust-v(?P<ver>[0-9]+\.[0-9]+\.[0-9]+)(-(alpha|beta)(\.[0-9]+)?)?$"
)


@dataclass(frozen=True)
class ReleaseMapping:
    repo: str
    tag: str
    version: str | None
    name: str | None
    draft: bool
    prerelease: bool
    created_at: str | None
    published_at: str | None
    html_url: str | None
    commit: str | None
    tag_present_locally: bool
    tag_ancestor_of_ref: bool | None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=DEFAULT_REPO, help="GitHub repo, e.g. openai/codex")
    parser.add_argument(
        "--remote",
        default=DEFAULT_REMOTE,
        help="Git remote name to fetch tags from (optional).",
    )
    parser.add_argument(
        "--fetch-tags",
        action="store_true",
        help="Run `git fetch <remote> --tags` before resolving tags.",
    )
    parser.add_argument(
        "--tag-prefix",
        default=DEFAULT_TAG_PREFIX,
        help="Only include releases whose tag starts with this prefix.",
    )
    parser.add_argument(
        "--semver-only",
        action="store_true",
        help="Only include rust-vX.Y.Z and rust-vX.Y.Z-(alpha|beta)(.N) tags.",
    )
    parser.add_argument(
        "--ref",
        default=None,
        help="Optional git ref to test ancestry against (e.g. upstream/main).",
    )
    parser.add_argument(
        "--allow-missing-tags",
        action="store_true",
        help="Do not fail if a release tag can't be resolved locally.",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=None,
        help="Maximum number of releases to return (after filtering).",
    )
    parser.add_argument(
        "--format",
        choices=("json", "jsonl", "tsv"),
        default="json",
        help="Output format.",
    )
    return parser.parse_args()


def _http_get_json(url: str) -> Any:
    headers = {
        "Accept": "application/vnd.github+json",
        "User-Agent": "codex-mine/scripts/map_upstream_releases.py",
    }
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(request, timeout=30) as resp:
        return json.loads(resp.read().decode("utf-8"))


def fetch_releases(repo: str) -> list[dict[str, Any]]:
    releases: list[dict[str, Any]] = []
    page = 1
    while True:
        url = f"{API_BASE}/repos/{repo}/releases?per_page=100&page={page}"
        try:
            batch = _http_get_json(url)
        except urllib.error.HTTPError as e:
            body = e.read().decode("utf-8", errors="replace") if hasattr(e, "read") else ""
            raise RuntimeError(f"GitHub API error ({e.code}) for {url}: {body}") from e
        if not isinstance(batch, list):
            raise RuntimeError(f"Unexpected GitHub API response for {url}: {type(batch)}")
        if not batch:
            break
        releases.extend(batch)
        page += 1
    return releases


def run_git(args: list[str]) -> str:
    return subprocess.check_output(["git", *args], text=True).strip()


def try_git(args: list[str]) -> tuple[bool, str]:
    try:
        out = run_git(args)
    except subprocess.CalledProcessError as e:
        return False, e.output.strip() if e.output else ""
    return True, out


def resolve_tag_commit(tag: str) -> tuple[bool, str | None]:
    ok, out = try_git(["rev-list", "-n1", tag])
    if not ok or not out:
        return False, None
    return True, out


def is_ancestor(tag: str, ref: str) -> bool | None:
    ok, _ = try_git(["merge-base", "--is-ancestor", tag, ref])
    if ok:
        return True
    # Distinguish "not ancestor" vs "tag/ref doesn't exist" is messy; keep it simple:
    # if tag exists but not ancestor, merge-base returns exit 1; if missing, also non-zero.
    # Here we only use this field as a hint.
    return False


def normalize_version(tag: str) -> str | None:
    if not tag.startswith(DEFAULT_TAG_PREFIX):
        return None
    return tag.removeprefix(DEFAULT_TAG_PREFIX)


def iter_mappings(
    releases: Iterable[dict[str, Any]],
    *,
    repo: str,
    tag_prefix: str,
    semver_only: bool,
    ref: str | None,
) -> Iterable[ReleaseMapping]:
    for rel in releases:
        tag = rel.get("tag_name")
        if not isinstance(tag, str):
            continue
        if not tag.startswith(tag_prefix):
            continue
        if semver_only and SEMVER_TAG_RE.match(tag) is None:
            continue

        tag_present, commit = resolve_tag_commit(tag)
        ancestor = is_ancestor(tag, ref) if (ref and tag_present) else None

        yield ReleaseMapping(
            repo=repo,
            tag=tag,
            version=normalize_version(tag) if tag.startswith(DEFAULT_TAG_PREFIX) else None,
            name=rel.get("name") if isinstance(rel.get("name"), str) else None,
            draft=bool(rel.get("draft")),
            prerelease=bool(rel.get("prerelease")),
            created_at=rel.get("created_at") if isinstance(rel.get("created_at"), str) else None,
            published_at=rel.get("published_at") if isinstance(rel.get("published_at"), str) else None,
            html_url=rel.get("html_url") if isinstance(rel.get("html_url"), str) else None,
            commit=commit,
            tag_present_locally=tag_present,
            tag_ancestor_of_ref=ancestor,
        )


def write_json(mappings: list[ReleaseMapping]) -> None:
    print(json.dumps([asdict(m) for m in mappings], indent=2, sort_keys=True))


def write_jsonl(mappings: list[ReleaseMapping]) -> None:
    for m in mappings:
        print(json.dumps(asdict(m), sort_keys=True))


def write_tsv(mappings: list[ReleaseMapping]) -> None:
    header = [
        "tag",
        "commit",
        "published_at",
        "prerelease",
        "draft",
        "name",
        "html_url",
        "tag_present_locally",
        "tag_ancestor_of_ref",
    ]
    print("\t".join(header))
    for m in mappings:
        row = [
            m.tag,
            m.commit or "",
            m.published_at or "",
            "1" if m.prerelease else "0",
            "1" if m.draft else "0",
            m.name or "",
            m.html_url or "",
            "1" if m.tag_present_locally else "0",
            "" if m.tag_ancestor_of_ref is None else ("1" if m.tag_ancestor_of_ref else "0"),
        ]
        print("\t".join(row))


def main() -> int:
    args = parse_args()

    if args.fetch_tags:
        subprocess.run(
            ["git", "fetch", args.remote, "--tags"],
            check=True,
        )

    if args.ref:
        ok, _ = try_git(["rev-parse", "--verify", f"{args.ref}^{{commit}}"])
        if not ok:
            raise RuntimeError(f"Unknown git ref: {args.ref}")

    releases = fetch_releases(args.repo)
    mappings = list(
        iter_mappings(
            releases,
            repo=args.repo,
            tag_prefix=args.tag_prefix,
            semver_only=args.semver_only,
            ref=args.ref,
        )
    )

    if args.limit is not None:
        mappings = mappings[: args.limit]

    missing = [m for m in mappings if not m.tag_present_locally]
    if missing and not args.allow_missing_tags:
        tags = ", ".join(m.tag for m in missing[:10])
        raise RuntimeError(
            f"Some release tags could not be resolved locally (try --fetch-tags, or --allow-missing-tags). "
            f"Missing: {tags}"
        )

    if args.format == "json":
        write_json(mappings)
    elif args.format == "jsonl":
        write_jsonl(mappings)
    elif args.format == "tsv":
        write_tsv(mappings)
    else:
        raise RuntimeError(f"Unknown format: {args.format}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
