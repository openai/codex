#!/usr/bin/env python3

import argparse
import base64
import json
import shlex
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Optional
from urllib.parse import quote


DEFAULT_SOURCE_REPO = "openai/codex"
COPIED_PREFIXES = (".github/actions/", ".github/workflows/")
COPIED_PATHS_DESCRIPTION = ".github/actions and .github/workflows"
ROOT = Path(__file__).resolve().parents[2]


@dataclass(frozen=True)
class Repo:
    owner: str
    name: str

    @property
    def slug(self) -> str:
        return f"{self.owner}/{self.name}"


@dataclass(frozen=True)
class GitHubChange:
    path: Path
    status: str
    blob_sha: Optional[str]
    previous_path: Optional[Path]


def main() -> int:
    args = parse_args()
    repo = parse_repo(args.repo)
    gh = find_gh()
    commit = load_commit(gh, repo, args.commit)
    changes = github_changes(commit)

    if not changes:
        print(
            f"No {COPIED_PATHS_DESCRIPTION} changes found in "
            f"{repo.slug}@{args.commit}."
        )
        return 0

    for change in changes:
        apply_change(gh, repo, change, args.destination_root, args.dry_run)

    action = "Would copy" if args.dry_run else "Copied"
    print(
        f"{action} {len(changes)} GitHub automation change(s) from "
        f"{repo.slug}@{commit['sha']}."
    )
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            f"Copy {COPIED_PATHS_DESCRIPTION} changes from one openai/codex commit "
            "into this checkout as unstaged changes."
        )
    )
    parser.add_argument("commit", help="Commit SHA or ref in the source repository.")
    parser.add_argument(
        "--repo",
        default=DEFAULT_SOURCE_REPO,
        help=f"Source GitHub repository. Defaults to {DEFAULT_SOURCE_REPO}.",
    )
    parser.add_argument(
        "--destination-root",
        type=Path,
        default=ROOT,
        help="Repository root to write into. Defaults to the checkout containing this script.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the files that would be written or removed without changing the checkout.",
    )
    return parser.parse_args()


def parse_repo(value: str) -> Repo:
    parts = value.split("/", 1)
    if len(parts) != 2 or not all(parts):
        raise RuntimeError(f"Repository must be formatted as owner/name: {value}")
    return Repo(owner=parts[0], name=parts[1])


def find_gh() -> str:
    gh = shutil.which("gh")
    if gh:
        return gh

    homebrew_gh = Path("/opt/homebrew/bin/gh")
    if homebrew_gh.exists():
        return homebrew_gh.as_posix()

    raise RuntimeError("Unable to find gh on PATH.")


def load_commit(gh: str, repo: Repo, commit: str) -> dict:
    return json.loads(
        output(
            [
                gh,
                "api",
                "-H",
                "Accept: application/vnd.github+json",
                f"/repos/{repo.owner}/{repo.name}/commits/{quote(commit, safe='')}",
            ]
        )
    )


def github_changes(commit: dict) -> list[GitHubChange]:
    changes = []
    for file in commit.get("files", []):
        path = Path(file["filename"])
        previous_path = previous_github_path(file)
        if not is_copied_path(path) and previous_path is None:
            continue

        changes.append(
            GitHubChange(
                path=path,
                status=file["status"],
                blob_sha=file.get("sha"),
                previous_path=previous_path,
            )
        )

    return changes


def previous_github_path(file: dict) -> Optional[Path]:
    previous_filename = file.get("previous_filename")
    if not previous_filename:
        return None

    path = Path(previous_filename)
    return path if is_copied_path(path) else None


def is_copied_path(path: Path) -> bool:
    return path.as_posix().startswith(COPIED_PREFIXES)


def apply_change(
    gh: str,
    repo: Repo,
    change: GitHubChange,
    destination_root: Path,
    dry_run: bool,
) -> None:
    if change.previous_path is not None and change.previous_path != change.path:
        remove_local_file(destination_root / change.previous_path, dry_run)

    if not is_copied_path(change.path):
        return

    if change.status == "removed":
        remove_local_file(destination_root / change.path, dry_run)
        return

    if change.blob_sha is None:
        raise RuntimeError(f"GitHub did not report a blob SHA for {change.path}.")

    content = load_blob(gh, repo, change.blob_sha)
    write_local_file(destination_root / change.path, content, dry_run)
    print(f"{change.status}: {change.path}")


def remove_local_file(path: Path, dry_run: bool) -> None:
    if dry_run:
        print(f"remove: {path}")
        return

    if path.exists():
        path.unlink()
        print(f"removed: {path}")
    else:
        print(f"already absent: {path}")


def write_local_file(path: Path, content: bytes, dry_run: bool) -> None:
    if dry_run:
        print(f"write: {path}")
        return

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)


def load_blob(gh: str, repo: Repo, blob_sha: str) -> bytes:
    blob = json.loads(
        output(
            [
                gh,
                "api",
                "-H",
                "Accept: application/vnd.github+json",
                f"/repos/{repo.owner}/{repo.name}/git/blobs/{blob_sha}",
            ]
        )
    )
    if blob.get("encoding") != "base64":
        raise RuntimeError(
            f"Unexpected blob encoding for {repo.slug}@{blob_sha}: "
            f"{blob.get('encoding')}"
        )

    return base64.b64decode(blob["content"])


def output(args: list[str]) -> str:
    return run(args, capture=True).stdout.strip()


def run(args: list[str], *, capture: bool = False) -> subprocess.CompletedProcess:
    print(f"+ {shlex.join(args)}", flush=True)
    completed = subprocess.run(
        args,
        check=False,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )
    if completed.returncode != 0:
        command = shlex.join(args)
        output_text = completed.stdout.strip() if completed.stdout else ""
        message = f"Command failed with exit code {completed.returncode}: {command}"
        if output_text:
            message = f"{message}\n{output_text}"
        raise RuntimeError(message)
    return completed


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1)
