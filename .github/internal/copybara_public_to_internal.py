#!/usr/bin/env python3

import json
import os
import shlex
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path

PUBLIC_REPO_URL = "https://github.com/openai/codex.git"
INTERNAL_REPO_URL = "https://github.com/openai/codex-internal"
SYNC_BRANCH = os.environ.get("SYNC_BRANCH", "copybara/public-to-internal")
TRAILER = "Codex-Public-RevId"
CARGO_LOCKFILE = Path("codex-rs/Cargo.lock")
EMPTY_IMPORT_MARKER_FILE = Path(".github/internal/last_empty_public_import.txt")
GENERATED_SYNC_PATHS = {CARGO_LOCKFILE, EMPTY_IMPORT_MARKER_FILE}


@dataclass(frozen=True)
class GitAuthor:
    name: str
    email: str
    date: str


@dataclass(frozen=True)
class PublicChange:
    rev: str
    author: GitAuthor
    title: str
    body: str
    url: str | None
    number: int | None


def main() -> int:
    github_token = required_env("GITHUB_TOKEN")
    copybara_jar = required_env("COPYBARA_JAR")
    public_ref = os.environ.get("PUBLIC_REF", "main")

    configure_git(github_token)
    ensure_public_remote()
    target_public_rev = fetch_public_target(public_ref)

    imported_count = 0
    while True:
        sync_internal_main()
        last_public_rev = find_last_public_rev() or find_initial_public_rev()

        if not last_public_rev:
            raise RuntimeError(
                "Unable to determine the last public revision imported into openai/codex-internal."
            )

        if not is_ancestor(last_public_rev, target_public_rev):
            raise RuntimeError(
                f"{target_public_rev} is not a descendant of last imported public "
                f"revision {last_public_rev}."
            )

        public_change_rev = next_public_change(last_public_rev, target_public_rev)
        if not public_change_rev:
            print(f"openai/codex is already imported through {target_public_rev}.")
            break

        change = load_public_change(public_change_rev)
        body_file, message_file = write_metadata_files(change)
        internal_cargo_lockfile = save_internal_cargo_lockfile(change)
        migrate_change(
            copybara_jar,
            last_public_rev,
            change,
            message_file,
            internal_cargo_lockfile,
        )
        pr_number = open_or_update_pr(change, body_file, message_file)
        merge_pr(pr_number, change, body_file)
        wait_for_imported_rev(change.rev)
        imported_count += 1

    print(f"Imported {imported_count} public change(s).")
    return 0


def required_env(name: str) -> str:
    value = os.environ.get(name)
    if not value:
        raise RuntimeError(f"{name} must be set")
    return value


def configure_git(github_token: str) -> None:
    run(["git", "config", "--global", "user.name", "github-actions[bot]"])
    run(
        [
            "git",
            "config",
            "--global",
            "user.email",
            "41898282+github-actions[bot]@users.noreply.github.com",
        ]
    )
    credential_url = f"https://x-access-token:{github_token}@github.com/openai/codex-internal"
    redacted_credential_url = "https://x-access-token:***@github.com/openai/codex-internal"
    run(
        [
            "git",
            "config",
            "--global",
            f"url.{credential_url}.insteadOf",
            INTERNAL_REPO_URL,
        ],
        display_args=[
            "git",
            "config",
            "--global",
            f"url.{redacted_credential_url}.insteadOf",
            INTERNAL_REPO_URL,
        ],
    )


def ensure_public_remote() -> None:
    remote = run(["git", "remote", "get-url", "public"], check=False, capture=True)
    if remote.returncode == 0:
        run(["git", "remote", "set-url", "public", PUBLIC_REPO_URL])
    else:
        run(["git", "remote", "add", "public", PUBLIC_REPO_URL])


def fetch_public_target(public_ref: str) -> str:
    run(["git", "fetch", "--no-tags", "public", "main:refs/remotes/public/main"])
    if public_ref == "main":
        return output(["git", "rev-parse", "refs/remotes/public/main"])

    run(["git", "fetch", "--no-tags", "public", public_ref])
    return output(["git", "rev-parse", "FETCH_HEAD"])


def sync_internal_main() -> None:
    run(["git", "fetch", "origin", "refs/heads/main:refs/remotes/origin/main"])
    run(["git", "checkout", "-B", "main", "refs/remotes/origin/main"])


def find_last_public_rev() -> str | None:
    message_log = output(["git", "log", "--format=%B", "HEAD"])
    prefix = f"{TRAILER}: "
    for line in message_log.splitlines():
        if line.startswith(prefix):
            return line[len(prefix) :].strip()
    return None


def find_initial_public_rev() -> str | None:
    merge_base = run(
        ["git", "merge-base", "HEAD", "refs/remotes/public/main"],
        check=False,
        capture=True,
    )
    if merge_base.returncode != 0:
        return None
    return merge_base.stdout.strip() or None


def is_ancestor(ancestor: str, descendant: str) -> bool:
    return (
        run(
            ["git", "merge-base", "--is-ancestor", ancestor, descendant],
            check=False,
        ).returncode
        == 0
    )


def next_public_change(last_public_rev: str, target_public_rev: str) -> str | None:
    revs = output(
        ["git", "rev-list", "--first-parent", f"{last_public_rev}..{target_public_rev}"]
    ).splitlines()
    if not revs:
        return None
    return revs[-1]


def load_public_change(public_change_rev: str) -> PublicChange:
    author = load_public_author(public_change_rev)
    pulls_json = output(
        [
            "gh",
            "api",
            "-H",
            "Accept: application/vnd.github+json",
            f"/repos/openai/codex/commits/{public_change_rev}/pulls",
        ]
    )
    pulls = json.loads(pulls_json)
    if pulls:
        pull = pulls[0]
        return PublicChange(
            rev=public_change_rev,
            author=author,
            title=pull["title"],
            body=pull.get("body") or "",
            url=pull["html_url"],
            number=pull["number"],
        )

    short_rev = public_change_rev[:12]
    return PublicChange(
        rev=public_change_rev,
        author=author,
        title=f"Sync openai/codex {short_rev}",
        body="",
        url=f"https://github.com/openai/codex/commit/{public_change_rev}",
        number=None,
    )


def load_public_author(public_change_rev: str) -> GitAuthor:
    raw_author = output(
        ["git", "show", "--no-patch", "--format=%an%x00%ae%x00%aI", public_change_rev]
    )
    parts = raw_author.split("\0")
    if len(parts) != 3 or not all(parts):
        raise RuntimeError(
            f"Unable to read author metadata for public commit {public_change_rev}."
        )
    return GitAuthor(name=parts[0], email=parts[1], date=parts[2])


def write_metadata_files(change: PublicChange) -> tuple[Path, Path]:
    runner_temp = Path(os.environ.get("RUNNER_TEMP", tempfile.gettempdir()))
    body_file = runner_temp / f"public-to-internal-{change.rev[:12]}-pr-body.md"
    message_file = runner_temp / f"public-to-internal-{change.rev[:12]}-message.md"

    if change.number is not None and change.url:
        body = (
            f"Synced from [openai/codex#{change.number}]({change.url}).\n\n"
            f"{change.body}\n\n"
            f"{TRAILER}: {change.rev}\n"
        )
    else:
        short_rev = change.rev[:12]
        body = (
            f"Synced from public commit [{short_rev}]({change.url}).\n\n{TRAILER}: {change.rev}\n"
        )

    body_file.write_text(body, encoding="utf-8")
    message_file.write_text(f"{change.title}\n\n{body}", encoding="utf-8")
    return body_file, message_file


def save_internal_cargo_lockfile(change: PublicChange) -> Path | None:
    if public_change_touched_path(change.rev, CARGO_LOCKFILE):
        print(
            f"{CARGO_LOCKFILE} changed in {change.rev}; "
            "using the public lockfile as the base."
        )
        return None

    runner_temp = Path(os.environ.get("RUNNER_TEMP", tempfile.gettempdir()))
    cargo_lockfile = runner_temp / f"internal-cargo-lock-{change.rev[:12]}.lock"
    cargo_lockfile.write_text(
        run(["git", "show", f"HEAD:{CARGO_LOCKFILE}"], capture=True).stdout,
        encoding="utf-8",
    )
    print(f"Saved current internal {CARGO_LOCKFILE} for lockfile regeneration.")
    return cargo_lockfile


def public_change_touched_path(public_change_rev: str, path: Path) -> bool:
    return path in public_change_touched_paths(public_change_rev)


def public_change_touched_paths(public_change_rev: str) -> list[Path]:
    return [
        Path(changed_path)
        for changed_path in output(
            [
                "git",
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "-r",
                public_change_rev,
            ]
        ).splitlines()
    ]


def public_change_touched_workflows(public_change_rev: str) -> list[Path]:
    return [
        path
        for path in public_change_touched_paths(public_change_rev)
        if path.as_posix().startswith(".github/workflows/")
    ]


def workflow_changes_command(public_change_rev: str) -> str:
    return shlex.join(
        [
            ".github/internal/copy_public_workflow_changes.py",
            public_change_rev,
        ]
    )


def migrate_change(
    copybara_jar: str,
    last_public_rev: str,
    change: PublicChange,
    message_file: Path,
    internal_cargo_lockfile: Path | None,
) -> None:
    if not public_change_touched_paths(change.rev):
        create_empty_import_marker_commit(change, message_file)
        return

    migration = run_copybara_migrate(
        [
            "java",
            "-jar",
            copybara_jar,
            "migrate",
            ".copybara/copy.bara.sky",
            "public_to_internal",
            change.rev,
            f"--last-rev={last_public_rev}",
            "--git-destination-non-fast-forward",
        ]
    )
    if copybara_empty_change(migration):
        create_empty_import_marker_commit(change, message_file)
        return

    if not fetch_sync_branch(check=False):
        create_empty_import_marker_commit(change, message_file)
        return

    run(["git", "checkout", "--detach", f"origin/{SYNC_BRANCH}"])
    resolve_cargo_lockfile(internal_cargo_lockfile)
    run(
        [
            "git",
            "commit",
            "--amend",
            "--allow-empty",
            "--no-verify",
            "--author",
            f"{change.author.name} <{change.author.email}>",
            "--date",
            change.author.date,
            "--file",
            str(message_file),
        ]
    )
    run(
        [
            "git",
            "push",
            "--force-with-lease",
            "origin",
            f"HEAD:refs/heads/{SYNC_BRANCH}",
        ]
    )


def run_copybara_migrate(args: list[str]) -> subprocess.CompletedProcess[str]:
    print(f"+ {shlex.join(args)}", flush=True)
    completed = subprocess.run(
        args,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if completed.stdout:
        print(completed.stdout, end="" if completed.stdout.endswith("\n") else "\n")
    if completed.returncode != 0 and not copybara_empty_change(completed):
        raise RuntimeError(
            f"Command failed with exit code {completed.returncode}: {shlex.join(args)}"
        )
    return completed


def copybara_empty_change(completed: subprocess.CompletedProcess[str]) -> bool:
    output_text = completed.stdout or ""
    return completed.returncode == 4 and (
        "resulted in an empty change in the destination" in output_text
        or "produced no changes in the destination" in output_text
    )


def resolve_cargo_lockfile(internal_cargo_lockfile: Path | None) -> None:
    if internal_cargo_lockfile is not None:
        CARGO_LOCKFILE.write_text(
            internal_cargo_lockfile.read_text(encoding="utf-8"),
            encoding="utf-8",
        )
    run(
        [
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--manifest-path",
            "codex-rs/Cargo.toml",
        ],
        capture=True,
    )
    run(["git", "add", CARGO_LOCKFILE.as_posix()])


def open_or_update_pr(change: PublicChange, body_file: Path, message_file: Path) -> str:
    run(["git", "fetch", "origin", "refs/heads/main:refs/remotes/origin/main"])
    fetch_sync_branch()

    ahead_count = int(output(["git", "rev-list", "--count", f"origin/main..origin/{SYNC_BRANCH}"]))
    if ahead_count == 0:
        create_empty_import_marker_commit_if_unchanged(change, message_file)
        fetch_sync_branch()
        ahead_count = int(output(["git", "rev-list", "--count", f"origin/main..origin/{SYNC_BRANCH}"]))
        if ahead_count == 0:
            raise RuntimeError(f"Copybara produced no commits on {SYNC_BRANCH}.")

    validate_sync_branch_paths(change.rev)

    pr_number = find_open_pr()
    if pr_number is None:
        run(
            [
                "gh",
                "pr",
                "create",
                "--base",
                "main",
                "--head",
                SYNC_BRANCH,
                "--title",
                change.title,
                "--body-file",
                str(body_file),
            ]
        )
        pr_number = find_open_pr()
    else:
        run(
            [
                "gh",
                "pr",
                "edit",
                pr_number,
                "--title",
                change.title,
                "--body-file",
                str(body_file),
            ]
        )

    if pr_number is None:
        raise RuntimeError("Unable to determine sync PR number.")
    return pr_number


def validate_sync_branch_paths(public_change_rev: str) -> None:
    allowed_paths = (
        set(public_change_touched_paths(public_change_rev)) | GENERATED_SYNC_PATHS
    )
    changed_paths = {
        Path(path)
        for path in output(
            [
                "git",
                "diff",
                "--name-only",
                "origin/main",
                f"origin/{SYNC_BRANCH}",
            ]
        ).splitlines()
    }
    unexpected_paths = sorted(changed_paths - allowed_paths)
    if unexpected_paths:
        formatted_paths = "\n".join(
            f"  - {path.as_posix()}" for path in unexpected_paths
        )
        raise RuntimeError(
            f"Copybara changed paths not touched by public commit {public_change_rev}:\n"
            f"{formatted_paths}"
        )


def create_empty_import_marker_commit_if_unchanged(
    change: PublicChange, message_file: Path
) -> None:
    if not trees_match("origin/main", f"origin/{SYNC_BRANCH}"):
        raise RuntimeError(
            f"Copybara produced no commits on {SYNC_BRANCH}, but the sync branch "
            "differs from origin/main."
        )

    create_empty_import_marker_commit(change, message_file)


def create_empty_import_marker_commit(change: PublicChange, message_file: Path) -> None:
    print(
        f"Copybara produced no content changes for {change.rev}; creating an empty "
        f"{TRAILER} marker commit."
    )
    run(["git", "checkout", "--detach", "origin/main"])
    EMPTY_IMPORT_MARKER_FILE.write_text(f"{change.rev}\n", encoding="utf-8")
    run(["git", "add", EMPTY_IMPORT_MARKER_FILE.as_posix()])
    run(
        [
            "git",
            "commit",
            "--allow-empty",
            "--no-verify",
            "--author",
            f"{change.author.name} <{change.author.email}>",
            "--date",
            change.author.date,
            "--file",
            str(message_file),
        ]
    )
    run(
        [
            "git",
            "push",
            "--force-with-lease",
            "origin",
            f"HEAD:refs/heads/{SYNC_BRANCH}",
        ]
    )


def trees_match(left: str, right: str) -> bool:
    diff = run(["git", "diff", "--quiet", left, right], check=False)
    if diff.returncode == 0:
        return True
    if diff.returncode == 1:
        return False
    raise RuntimeError(f"Unable to compare trees for {left} and {right}.")


def find_open_pr() -> str | None:
    pr_number = output(
        [
            "gh",
            "pr",
            "list",
            "--base",
            "main",
            "--head",
            SYNC_BRANCH,
            "--state",
            "open",
            "--json",
            "number",
            "--jq",
            ".[0].number // empty",
        ]
    )
    return pr_number or None


def merge_pr(pr_number: str, change: PublicChange, body_file: Path) -> None:
    sync_head = output(["git", "rev-parse", f"origin/{SYNC_BRANCH}"])
    merge = run(
        [
            "gh",
            "pr",
            "merge",
            pr_number,
            "--rebase",
            "--admin",
            "--delete-branch",
            "--match-head-commit",
            sync_head,
        ],
        check=False,
    )
    if merge.returncode != 0:
        workflow_paths = public_change_touched_workflows(change.rev)
        if workflow_paths:
            formatted_paths = "\n".join(
                f"  - {path.as_posix()}" for path in workflow_paths
            )
            raise RuntimeError(
                f"Failed to merge sync PR #{pr_number} for {change.rev}. The merge "
                "may have failed because the public change touched "
                f".github/workflows:\n{formatted_paths}\n\n"
                "To copy the public workflow changes into this checkout as unstaged "
                "changes, run:\n"
                f"  {workflow_changes_command(change.rev)}"
            )

        raise RuntimeError(f"Failed to merge sync PR #{pr_number} for {change.rev}.")

    print(f"Merged sync PR #{pr_number} for {change.rev}.")


def wait_for_imported_rev(public_change_rev: str) -> None:
    for _ in range(12):
        sync_internal_main()
        if find_last_public_rev() == public_change_rev:
            return
        time.sleep(5)
    raise RuntimeError(f"openai/codex-internal main did not advance to {public_change_rev}.")


def fetch_sync_branch(*, check: bool = True) -> bool:
    result = run(
        [
            "git",
            "fetch",
            "origin",
            f"+refs/heads/{SYNC_BRANCH}:refs/remotes/origin/{SYNC_BRANCH}",
        ],
        check=check,
    )
    return result.returncode == 0


def output(args: list[str]) -> str:
    return run(args, capture=True).stdout.strip()


def run(
    args: list[str],
    *,
    check: bool = True,
    capture: bool = False,
    display_args: list[str] | None = None,
) -> subprocess.CompletedProcess[str]:
    display_args = display_args or args
    print(f"+ {shlex.join(display_args)}", flush=True)
    completed = subprocess.run(
        args,
        check=False,
        text=True,
        stdout=subprocess.PIPE if capture else None,
    )
    if check and completed.returncode != 0:
        command = shlex.join(display_args)
        raise RuntimeError(f"Command failed with exit code {completed.returncode}: {command}")
    return completed


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1)
