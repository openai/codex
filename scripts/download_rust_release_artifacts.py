#!/usr/bin/env python3
"""Download codex Rust release artifacts to your local machine.

This script finds the GitHub Actions run for the `rust-release` workflow that
corresponds to a given release tag (e.g. `rust-v0.78.0-alpha.9`) and downloads
its uploaded artifacts into a local directory.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from datetime import date, datetime
from pathlib import Path


DEFAULT_REPO = "khai-oai/codex"
WORKFLOW_FILE = ".github/workflows/rust-release.yml"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo",
        default=DEFAULT_REPO,
        help=f"GitHub repo in owner/name form (default: {DEFAULT_REPO}).",
    )
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument(
        "--tag",
        help="Release tag that triggered the workflow (e.g. rust-v0.78.0-alpha.9).",
    )
    group.add_argument(
        "--version",
        help="Release version (e.g. 0.78.0-alpha.9). Implies tag rust-v<VERSION>.",
    )
    group.add_argument(
        "--run-id",
        type=int,
        help="GitHub Actions run databaseId to download directly.",
    )
    parser.add_argument(
        "--label",
        default=None,
        help=(
            "Directory name to download into (default: today's date as YYYY-MM-DD). "
            "This is only used to name the local output folder."
        ),
    )
    parser.add_argument(
        "--dest",
        type=Path,
        default=None,
        help="Destination root directory (default: ./dist/rust-release).",
    )
    parser.add_argument(
        "--keep-temp",
        action="store_true",
        help="Do not delete the temporary download directory.",
    )
    parser.add_argument(
        "--include-failed",
        action="store_true",
        help="Allow selecting a failed/cancelled run if it is the most recent for the tag.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    gh = shutil.which("gh")
    if gh is None:
        print("error: `gh` is required (GitHub CLI).", file=sys.stderr)
        return 2

    repo = args.repo.strip()
    run_id: int

    if args.run_id is not None:
        run_id = int(args.run_id)
    else:
        tag = args.tag or f"rust-v{args.version}"
        run = resolve_rust_release_run(repo, tag, include_failed=args.include_failed)
        run_id = int(run["databaseId"])

    label = (args.label or date.today().isoformat()).strip()
    if not label:
        label = date.today().isoformat()

    dest_root = (args.dest or (Path.cwd() / "dist" / "rust-release")).resolve()
    dest = dest_root / label
    dest.mkdir(parents=True, exist_ok=True)

    temp_dir = dest / ".tmp-download"
    if temp_dir.exists():
        shutil.rmtree(temp_dir)
    temp_dir.mkdir(parents=True, exist_ok=True)

    try:
        run_command(
            [
                gh,
                "run",
                "download",
                str(run_id),
                "--repo",
                repo,
                "--dir",
                str(temp_dir),
            ]
        )

        # Move downloaded artifacts up one level for convenience.
        # `gh run download` typically creates one directory per artifact name.
        for entry in sorted(temp_dir.iterdir()):
            target = dest / entry.name
            if target.exists():
                shutil.rmtree(target) if target.is_dir() else target.unlink()
            shutil.move(str(entry), str(target))

        print(f"Downloaded artifacts to {dest}")
        return 0
    finally:
        if not args.keep_temp and temp_dir.exists():
            shutil.rmtree(temp_dir, ignore_errors=True)


def resolve_rust_release_run(repo: str, tag: str, *, include_failed: bool) -> dict:
    # For tag-triggered workflows, GitHub CLI reports the tag name in `headBranch`.
    stdout = subprocess.check_output(
        [
            "gh",
            "run",
            "list",
            "--repo",
            repo,
            "--workflow",
            WORKFLOW_FILE,
            "--branch",
            tag,
            "--limit",
            "20",
            "--json",
            "databaseId,status,conclusion,createdAt,url,headSha,headBranch",
        ],
        text=True,
    )
    runs = json.loads(stdout or "[]")

    def acceptable(run: dict) -> bool:
        if include_failed:
            return True
        return run.get("conclusion") in (None, "success")

    candidates = [run for run in runs if acceptable(run)]
    if not candidates:
        raise RuntimeError(
            f"No acceptable rust-release runs found for {repo}@{tag}. "
            "Try --include-failed, or pass --run-id."
        )

    # Sort newest first (createdAt is ISO 8601).
    candidates.sort(
        key=lambda r: datetime.fromisoformat(r["createdAt"].replace("Z", "+00:00")),
        reverse=True,
    )
    return candidates[0]


def run_command(cmd: list[str]) -> None:
    print("+", " ".join(cmd))
    subprocess.run(cmd, check=True)


if __name__ == "__main__":
    raise SystemExit(main())
