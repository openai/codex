#!/usr/bin/env python3

"""Stage one private Codex commit at a time for public export.

The candidate branch contains commits whose private-only code has already been filtered. The ready
branch contains the projected public tree plus an internal state file recording the last processed
candidate; the final export excludes that marker.

``prepare`` selects the oldest unprocessed first-parent candidate, removes internal staging files,
reconciles the public Cargo lockfile, and archives the projected tree. For message generation it
also creates a public-only Git bundle with two synthetic commits: the exact previous public tree and
the exact candidate tree with a placeholder message. A separate workflow imports that disconnected
branch into a full ``openai/codex`` clone, so Codex can inspect the target and real public history
without receiving internal Git objects or the original commit message.

``validate-message`` enforces the public message policy on either the file produced by Codex or a
reviewed manual override. ``publish`` rechecks branch state, applies the projected tree and
validated message to the ready branch, updates the state marker, and pushes with a lease so
concurrent runs cannot silently overwrite one another.
"""

import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
from contextlib import contextmanager
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterator

SUPPORT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, SUPPORT_DIR.as_posix())

# The workflow may inherit a sanitized PYTHONPATH, so resolve sibling support
# modules from the checked-out bundle before importing them.
from message_policy import render_public_references  # noqa: E402
from message_policy import validate_message  # noqa: E402

CANDIDATE_BRANCH = "copybara-no-internal-code"
READY_BRANCH = "copybara-no-internal-references"
GITHUB_DIR = Path(".github")
STATE_FILE = Path(".codex-internal-to-public-state")
CARGO_MANIFEST = Path("codex-rs/Cargo.toml")
CARGO_LOCKFILE = Path("codex-rs/Cargo.lock")
BOT_NAME = "OpenAI Codex Sync"
BOT_EMAIL = "codex-sync@openai.com"
MESSAGE_WORKSPACE_BRANCH = "public-message-workspace"
MESSAGE_WORKSPACE_BUNDLE = "message-workspace.bundle"
MESSAGE_TARGET_FILE = "target-commit.txt"
MESSAGE_BASELINE_SUBJECT = "Public staging baseline"
MESSAGE_PLACEHOLDER_SUBJECT = "PLACEHOLDER: write the public commit message"


@dataclass(frozen=True)
class GitAuthor:
    name: str
    email: str
    date: str


@dataclass(frozen=True)
class PublishMetadata:
    candidate_revision: str
    ready_parent: str
    author: GitAuthor


@dataclass(frozen=True)
class PublicMessageOverride:
    expected_candidate_revision: str
    subject: str
    body: str


def main() -> int:
    if len(sys.argv) != 2:
        raise RuntimeError(
            "expected one command: prepare, validate-message, or publish"
        )
    command = sys.argv[1]
    runner_temp = Path(required_env("RUNNER_TEMP"))
    public_message_file = runner_temp / "public-commit-message.md"
    github_output_value = os.environ.get("GITHUB_OUTPUT")
    github_output = Path(github_output_value) if github_output_value else None
    if command == "prepare":
        message_override = read_public_message_override()
        prepare(
            runner_temp / "public-projection.tgz",
            runner_temp / "message-input",
            runner_temp / "publish-metadata.json",
            github_output,
            message_override,
        )
    elif command == "validate-message":
        message_override = read_public_message_override()
        if message_override is None:
            message = public_message_file.read_text(encoding="utf-8")
        else:
            message = message_override.subject
            if message_override.body.strip():
                message = f"{message}\n\n{message_override.body}"
        validate_message(
            message,
            Path("public-history"),
            public_message_file,
        )
    elif command == "publish":
        publish(
            runner_temp / "public-projection.tgz",
            runner_temp / "publish-metadata.json",
            public_message_file,
            github_output,
        )
    else:
        raise RuntimeError(f"unknown command: {command}")
    return 0


def prepare(
    projection_archive: Path,
    message_input_dir: Path,
    metadata_file: Path,
    github_output: Path | None,
    message_override: PublicMessageOverride | None,
) -> None:
    fetch_branch(CANDIDATE_BRANCH)
    fetch_branch(READY_BRANCH)
    candidate_head = rev_parse(remote_ref(CANDIDATE_BRANCH))
    ready_parent = rev_parse(remote_ref(READY_BRANCH))
    last_candidate = read_state(ready_parent)
    ensure_first_parent_ancestor(last_candidate, candidate_head)
    candidate_revision = next_first_parent_revision(last_candidate, candidate_head)
    if candidate_revision is None:
        if message_override is not None:
            raise RuntimeError(
                "A public message override was supplied, but there is no pending "
                f"commit on {CANDIDATE_BRANCH}."
            )
        print(f"{READY_BRANCH} is already staged through {candidate_head}.")
        write_github_output(github_output, "has_change", "false")
        write_github_output(github_output, "message_override", "false")
        return
    if (
        message_override is not None
        and message_override.expected_candidate_revision != candidate_revision
    ):
        raise RuntimeError(
            "The public message override expected candidate "
            f"{message_override.expected_candidate_revision}, but the next pending "
            f"candidate is {candidate_revision}."
        )

    metadata = PublishMetadata(
        candidate_revision=candidate_revision,
        ready_parent=ready_parent,
        author=load_author(candidate_revision),
    )
    with tempfile.TemporaryDirectory(prefix="codex-public-stage-") as temp_dir:
        temp_root = Path(temp_dir)
        with git_worktree(
            candidate_revision, temp_root / "candidate"
        ) as candidate_tree:
            remove_github_files(candidate_tree)
            (candidate_tree / STATE_FILE).unlink(missing_ok=True)
            reject_internal_paths(candidate_tree)
            restore_previous_lockfile(ready_parent, candidate_tree)
            reject_internal_cargo_references(candidate_tree)
            reconcile_cargo_lockfile(candidate_tree)
            reject_internal_cargo_references(candidate_tree)
            write_projection_archive(candidate_tree, projection_archive)
            if message_override is None:
                write_message_inputs(
                    projection_archive,
                    ready_parent,
                    candidate_revision,
                    metadata.author,
                    message_input_dir,
                )

    write_metadata(metadata_file, metadata)
    write_github_output(github_output, "has_change", "true")
    write_github_output(
        github_output,
        "message_override",
        str(message_override is not None).lower(),
    )
    print(f"Prepared {candidate_revision} from {CANDIDATE_BRANCH}.")


def publish(
    projection_archive: Path,
    metadata_file: Path,
    message_file: Path,
    github_output: Path | None,
) -> None:
    metadata = read_metadata(metadata_file)
    fetch_branch(CANDIDATE_BRANCH)
    fetch_branch(READY_BRANCH)
    current_ready = rev_parse(remote_ref(READY_BRANCH))
    if current_ready != metadata.ready_parent:
        raise RuntimeError(
            f"{READY_BRANCH} advanced from {metadata.ready_parent} to {current_ready} "
            "after the public projection was prepared."
        )
    ensure_first_parent_ancestor(
        metadata.candidate_revision, rev_parse(remote_ref(CANDIDATE_BRANCH))
    )

    with tempfile.TemporaryDirectory(prefix="codex-public-publish-") as temp_dir:
        temp_root = Path(temp_dir)
        projection = temp_root / "projection"
        extract_projection_archive(projection_archive, projection)
        reject_internal_paths(projection)
        with git_worktree(metadata.ready_parent, temp_root / "ready") as ready_tree:
            replace_worktree(ready_tree, projection)
            state_file = ready_tree / STATE_FILE
            state_file.parent.mkdir(parents=True, exist_ok=True)
            state_file.write_text(f"{metadata.candidate_revision}\n", encoding="utf-8")
            commit_ready_change(ready_tree, metadata, message_file)
            new_revision = output(["git", "rev-parse", "HEAD"], cwd=ready_tree)
            run(
                [
                    "git",
                    "push",
                    f"--force-with-lease=refs/heads/{READY_BRANCH}:{metadata.ready_parent}",
                    "origin",
                    f"HEAD:refs/heads/{READY_BRANCH}",
                ],
                cwd=ready_tree,
            )

    write_github_output(github_output, "published", "true")
    write_github_output(github_output, "ready_revision", new_revision)
    print(
        f"Published {metadata.candidate_revision} as {new_revision} on {READY_BRANCH}."
    )


def next_first_parent_revision(last_revision: str, target_revision: str) -> str | None:
    revisions = output(
        [
            "git",
            "rev-list",
            "--first-parent",
            "--reverse",
            f"{last_revision}..{target_revision}",
        ]
    ).splitlines()
    return revisions[0] if revisions else None


def read_state(ready_revision: str) -> str:
    result = run(
        ["git", "show", f"{ready_revision}:{STATE_FILE.as_posix()}"],
        check=False,
        capture=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"{READY_BRANCH} does not contain {STATE_FILE}. Follow the bootstrap "
            "procedure in .github/codex-internal-to-public/README.md."
        )
    revision = result.stdout.strip()
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise RuntimeError(f"{STATE_FILE} does not contain one full Git commit SHA.")
    return revision


def read_public_message_override() -> PublicMessageOverride | None:
    enabled = os.environ.get("USE_PUBLIC_MESSAGE_OVERRIDE", "").strip().lower()
    expected_revision = os.environ.get("EXPECTED_CANDIDATE_REVISION", "").strip()
    subject = os.environ.get("PUBLIC_SUBJECT", "")
    body = os.environ.get("PUBLIC_BODY", "")
    supplied_values = (expected_revision, subject, body)

    if enabled not in {"", "false", "true"}:
        raise RuntimeError("USE_PUBLIC_MESSAGE_OVERRIDE must be true or false")
    if enabled != "true":
        if any(supplied_values):
            raise RuntimeError(
                "Set USE_PUBLIC_MESSAGE_OVERRIDE=true when supplying a public "
                "message override."
            )
        return None
    if not re.fullmatch(r"[0-9a-f]{40}", expected_revision):
        raise RuntimeError(
            "EXPECTED_CANDIDATE_REVISION must be the full lowercase SHA of the "
            "next pending candidate commit."
        )
    if not subject.strip():
        raise RuntimeError("PUBLIC_SUBJECT must be set for a public message override")
    return PublicMessageOverride(
        expected_candidate_revision=expected_revision,
        subject=subject,
        body=body,
    )


def restore_previous_lockfile(ready_revision: str, candidate_tree: Path) -> None:
    lockfile = output(["git", "show", f"{ready_revision}:{CARGO_LOCKFILE.as_posix()}"])
    (candidate_tree / CARGO_LOCKFILE).write_text(f"{lockfile}\n", encoding="utf-8")


def reconcile_cargo_lockfile(candidate_tree: Path) -> None:
    manifest = candidate_tree / CARGO_MANIFEST
    args = [
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--manifest-path",
        manifest.as_posix(),
    ]
    run(args, cwd=candidate_tree, capture=True)
    run([*args, "--locked"], cwd=candidate_tree, capture=True)


def reject_internal_cargo_references(candidate_tree: Path) -> None:
    paths = [candidate_tree / CARGO_LOCKFILE]
    paths.extend((candidate_tree / "codex-rs").rglob("Cargo.toml"))
    forbidden = re.compile(
        r"codex[-_]internal|github\.com/openai/codex-internal", re.IGNORECASE
    )
    for path in paths:
        match = forbidden.search(path.read_text(encoding="utf-8"))
        if match:
            raise RuntimeError(
                f"Public Cargo metadata in {path.relative_to(candidate_tree)} contains "
                f"an internal reference: {match.group(0)}"
            )


def remove_github_files(tree: Path) -> None:
    shutil.rmtree(tree / GITHUB_DIR, ignore_errors=True)


def reject_internal_paths(tree: Path) -> None:
    if (tree / GITHUB_DIR).exists():
        raise RuntimeError("The public projection unexpectedly contains .github.")
    if (tree / STATE_FILE).exists():
        raise RuntimeError(
            f"The public projection unexpectedly contains {STATE_FILE}."
        )


def write_message_inputs(
    projection_archive: Path,
    ready_parent: str,
    candidate_revision: str,
    author: GitAuthor,
    destination: Path,
) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="codex-public-message-") as temp_dir:
        temp_root = Path(temp_dir)
        baseline_archive = temp_root / "baseline.tgz"
        with git_worktree(ready_parent, temp_root / "ready-parent") as ready_tree:
            remove_github_files(ready_tree)
            (ready_tree / STATE_FILE).unlink(missing_ok=True)
            reject_internal_paths(ready_tree)
            reject_internal_cargo_references(ready_tree)
            write_projection_archive(ready_tree, baseline_archive)

        baseline = temp_root / "baseline"
        extract_projection_archive(baseline_archive, baseline)
        candidate = temp_root / "candidate"
        extract_projection_archive(projection_archive, candidate)

        workspace = temp_root / "message-workspace"
        run(
            [
                "git",
                "init",
                f"--initial-branch={MESSAGE_WORKSPACE_BRANCH}",
                workspace.as_posix(),
            ],
        )
        run(["git", "config", "user.name", BOT_NAME], cwd=workspace)
        run(["git", "config", "user.email", BOT_EMAIL], cwd=workspace)
        replace_worktree(workspace, baseline)
        run(["git", "add", "--all"], cwd=workspace)
        run(
            [
                "git",
                "commit",
                "--allow-empty",
                "--no-verify",
                "--message",
                MESSAGE_BASELINE_SUBJECT,
            ],
            cwd=workspace,
        )

        replace_worktree(workspace, candidate)
        run(["git", "add", "--all"], cwd=workspace)
        run(
            [
                "git",
                "commit",
                "--allow-empty",
                "--no-verify",
                "--author",
                f"{author.name} <{author.email}>",
                "--date",
                author.date,
                "--message",
                MESSAGE_PLACEHOLDER_SUBJECT,
            ],
            cwd=workspace,
        )
        target_commit = output(["git", "rev-parse", "HEAD"], cwd=workspace)
        run(
            [
                "git",
                "bundle",
                "create",
                (destination / MESSAGE_WORKSPACE_BUNDLE).resolve().as_posix(),
                f"refs/heads/{MESSAGE_WORKSPACE_BRANCH}",
            ],
            cwd=workspace,
        )
        (destination / MESSAGE_TARGET_FILE).write_text(
            f"{target_commit}\n", encoding="utf-8"
        )

    source_message = output(
        ["git", "show", "--no-patch", "--format=%B", candidate_revision]
    )
    (destination / "public-references.md").write_text(
        render_public_references(source_message), encoding="utf-8"
    )

    shutil.copy2(SUPPORT_DIR / "commit_message_prompt.md", destination / "prompt.md")
    shutil.copy2(SUPPORT_DIR / "message_policy.py", destination / "message_policy.py")


def write_projection_archive(tree: Path, archive: Path) -> None:
    archive.parent.mkdir(parents=True, exist_ok=True)
    run(["git", "add", "--all"], cwd=tree)
    tree_revision = output(["git", "write-tree"], cwd=tree)
    run(
        [
            "git",
            "archive",
            "--format=tar.gz",
            f"--output={archive.resolve()}",
            tree_revision,
        ],
        cwd=tree,
    )


def extract_projection_archive(archive: Path, destination: Path) -> None:
    destination.mkdir(parents=True)
    with tarfile.open(archive, "r:gz") as tar:
        tar.extractall(destination, filter="data")


def replace_worktree(worktree: Path, projection: Path) -> None:
    for child in worktree.iterdir():
        if child.name == ".git":
            continue
        if child.is_dir() and not child.is_symlink():
            shutil.rmtree(child)
        else:
            child.unlink()
    shutil.copytree(projection, worktree, dirs_exist_ok=True, symlinks=True)


def commit_ready_change(
    ready_tree: Path, metadata: PublishMetadata, message_file: Path
) -> None:
    run(["git", "config", "user.name", BOT_NAME], cwd=ready_tree)
    run(["git", "config", "user.email", BOT_EMAIL], cwd=ready_tree)
    run(["git", "add", "--all"], cwd=ready_tree)
    run(
        [
            "git",
            "commit",
            "--allow-empty",
            "--no-verify",
            "--author",
            f"{metadata.author.name} <{metadata.author.email}>",
            "--date",
            metadata.author.date,
            "--file",
            message_file.resolve().as_posix(),
        ],
        cwd=ready_tree,
    )


def load_author(revision: str) -> GitAuthor:
    raw = output(["git", "show", "--no-patch", "--format=%an%x00%ae%x00%aI", revision])
    parts = raw.split("\0")
    if len(parts) != 3 or not all(parts):
        raise RuntimeError(f"Unable to read author metadata for {revision}.")
    return GitAuthor(name=parts[0], email=parts[1], date=parts[2])


def write_metadata(path: Path, metadata: PublishMetadata) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(asdict(metadata), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def read_metadata(path: Path) -> PublishMetadata:
    value = json.loads(path.read_text(encoding="utf-8"))
    return PublishMetadata(
        candidate_revision=value["candidate_revision"],
        ready_parent=value["ready_parent"],
        author=GitAuthor(**value["author"]),
    )


def fetch_branch(branch: str) -> None:
    result = run(
        [
            "git",
            "fetch",
            "--no-tags",
            "origin",
            f"+refs/heads/{branch}:refs/remotes/origin/{branch}",
        ],
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"Unable to fetch {branch}. If it has not been created yet, follow the "
            "bootstrap procedure in .github/codex-internal-to-public/README.md."
        )


def remote_ref(branch: str) -> str:
    return f"refs/remotes/origin/{branch}"


def rev_parse(revision: str) -> str:
    return output(["git", "rev-parse", f"{revision}^{{commit}}"])


def ensure_first_parent_ancestor(ancestor: str, descendant: str) -> None:
    first_parent_revisions = output(
        ["git", "rev-list", "--first-parent", descendant]
    ).splitlines()
    if ancestor not in first_parent_revisions:
        raise RuntimeError(
            f"{ancestor} is not on the first-parent chain of {descendant}."
        )


@contextmanager
def git_worktree(revision: str, path: Path) -> Iterator[Path]:
    run(["git", "worktree", "add", "--detach", path.as_posix(), revision])
    try:
        yield path
    finally:
        run(["git", "worktree", "remove", "--force", path.as_posix()], check=False)


def write_github_output(path: Path | None, name: str, value: str) -> None:
    if path is None:
        return
    with path.open("a", encoding="utf-8") as output_file:
        output_file.write(f"{name}={value}\n")


def required_env(name: str) -> str:
    value = os.environ.get(name)
    if not value:
        raise RuntimeError(f"{name} must be set")
    return value


def output(args: list[str], *, cwd: Path | None = None, strip: bool = True) -> str:
    value = run(args, cwd=cwd, capture=True).stdout
    return value.strip() if strip else value


def run(
    args: list[str],
    *,
    cwd: Path | None = None,
    check: bool = True,
    capture: bool = False,
) -> subprocess.CompletedProcess[str]:
    print(f"+ {shlex.join(args)}", flush=True)
    completed = subprocess.run(
        args,
        cwd=cwd,
        check=False,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )
    if check and completed.returncode != 0:
        detail = f"\n{completed.stdout}" if capture and completed.stdout else ""
        raise RuntimeError(
            f"Command failed with exit code {completed.returncode}: "
            f"{shlex.join(args)}{detail}"
        )
    return completed


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1)
