#!/usr/bin/env python3

import argparse
import asyncio
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass

"""
Strategy

This script generates GitHub release notes for a tag like `rust-v0.77.0` using only `gh`
(no local `git`), because Codex release tags may not be on the default branch.

High-level flow:
- Normalize the user-provided tag (accepts `rust-vX.Y.Z`, `vX.Y.Z`, or `X.Y.Z`).
- Determine the previous release tag (by default: subtract 1 from the minor version,
  with an escape hatch via `--prev-tag` for scrubbed/irregular releases).
- Resolve each tag to its tagged commit, then select the parent commit that is on the
  repo's default branch. We diff between those parent commits so we get the same commit
  range that landed on main, without including the ancestor itself (previous release).
- Use the GitHub compare API to list commits in the range and GraphQL to map those
  commits to associated PRs (deduplicated).
- Emit a raw changelog (`changelog.md`) that includes a compare URL and one bullet per PR.
- Fetch extra PR details (with optional caching + bounded concurrency), write PR JSON to
  disk, and feed a condensed JSON payload into `codex exec` to produce an *overview* of
  highlighted changes (`overview.md`).
- Concatenate `overview.md` + `changelog.md` into `release_notes.md` for GitHub Releases.
"""


DEFAULT_REPO = "openai/codex"
GH_TIMEOUT_SECONDS = 60
CODEX_TIMEOUT_SECONDS = 20 * 60
DEFAULT_MAX_CONCURRENCY = 6
DEFAULT_COMMENTS_LIMIT = 100


@dataclass(frozen=True)
class Repo:
    owner: str
    name: str


@dataclass(frozen=True)
class PullRequest:
    number: int
    title: str
    author_login: str | None


class ReleaseNotesError(RuntimeError):
    pass


def log(message: str) -> None:
    print(f"[release-notes] {message}", file=sys.stderr)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Generate release notes markdown for a GitHub release tag by listing PRs "
            "introduced since the previous release."
        )
    )
    parser.add_argument(
        "tag",
        help="Release tag, e.g. rust-v0.77.0 (also accepts v0.77.0 or 0.77.0).",
    )
    parser.add_argument(
        "--head-sha",
        help=(
            "Optional commit SHA on the default branch to use as the 'new release' end of the diff. "
            "This is useful when generating notes before creating the tag."
        ),
    )
    parser.add_argument(
        "--prev-tag",
        help=(
            "Previous release tag to diff against. If omitted, it is derived by "
            "subtracting 1 from the minor version in the semver part of the provided tag."
        ),
    )
    parser.add_argument(
        "--repo",
        default=DEFAULT_REPO,
        help=f"GitHub repo in OWNER/NAME form. (default: {DEFAULT_REPO})",
    )
    parser.add_argument(
        "--prs-dir",
        help=(
            "Optional directory to cache PR JSON payloads across runs. When set, the script "
            "reuses existing `NUMBER.json` files and only fetches missing PRs."
        ),
    )
    parser.add_argument(
        "--max-concurrency",
        type=int,
        default=DEFAULT_MAX_CONCURRENCY,
        help=f"Max concurrent `gh api` requests when fetching PR details. (default: {DEFAULT_MAX_CONCURRENCY})",
    )
    parser.add_argument(
        "--comments-limit",
        type=int,
        default=DEFAULT_COMMENTS_LIMIT,
        help=(
            "How many issue comments to fetch per PR (most recent first). Set to 0 to skip. "
            f"(default: {DEFAULT_COMMENTS_LIMIT})"
        ),
    )
    parser.add_argument(
        "-o",
        "--output",
        help=(
            "Optional path to copy the generated `release_notes.md` to. If this is a directory, "
            "the file will be copied as `release_notes.md` inside it."
        ),
    )
    return parser.parse_args(argv[1:])


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    repo = parse_repo(args.repo)
    out_dir = tempfile.mkdtemp(prefix="codex-release-notes-")
    pr_cache_dir = args.prs_dir
    max_concurrency = args.max_concurrency
    comments_limit = args.comments_limit
    output = args.output
    head_sha = args.head_sha

    try:
        tag = normalize_release_tag(args.tag)
        if args.prev_tag:
            prev_tag = normalize_release_tag(args.prev_tag)
        else:
            prev_tag = derive_prev_tag(tag)

        log(f"Output directory: {out_dir}")
        log(f"Release tag: {tag}")
        log(f"Previous tag: {prev_tag}")
        if head_sha:
            log(f"Head SHA override: {head_sha}")
        if pr_cache_dir:
            log(f"PR cache directory: {pr_cache_dir}")
        log(f"Max concurrency: {max_concurrency}")
        log(f"Comments limit: {comments_limit}")

        default_branch_head = get_default_branch_head(repo)
        log(f"Default branch head: {default_branch_head}")

        if head_sha:
            head_parent = head_sha
        else:
            head_parent = get_tag_parent_commit(repo, tag, default_branch_head)
        base_parent = get_tag_parent_commit(repo, prev_tag, default_branch_head)
        log(f"Release base commit (parent on default branch): {head_parent}")
        log(f"Previous base commit (parent on default branch): {base_parent}")

        log("Fetching commit list via GitHub compare API…")
        commit_shas = compare_commits(repo, base_parent, head_parent)
        log(f"Found {len(commit_shas)} commits in range.")

        log("Mapping commits -> associated PRs via GitHub GraphQL…")
        prs = collect_prs_for_commits(repo, commit_shas)
        log(f"Found {len(prs)} unique PRs in range.")

        changelog_md = render_changelog(repo, prev_tag, tag, prs)
        changelog_path = os.path.join(out_dir, "changelog.md")
        log(f"Writing changelog to: {changelog_path}")
        write_text_file(changelog_path, changelog_md)

        pr_out_dir = os.path.join(out_dir, "prs")
        os.makedirs(pr_out_dir, exist_ok=True)
        if pr_cache_dir:
            os.makedirs(pr_cache_dir, exist_ok=True)
            log(f"Fetching PR details (reusing cache), writing JSON to: {pr_cache_dir}")
        else:
            pr_cache_dir = pr_out_dir
            log(f"Fetching PR details + files, writing JSON to: {pr_out_dir}")

        pr_summaries = asyncio.run(
            fetch_and_write_pr_details(
                repo, pr_cache_dir, pr_out_dir, prs, max_concurrency, comments_limit
            )
        )
        pr_index_path = os.path.join(out_dir, "prs_index.json")
        log(f"Writing PR index to: {pr_index_path}")
        write_text_file(
            pr_index_path, json.dumps(pr_summaries, indent=2, sort_keys=True) + "\n"
        )

        prompt_path = os.path.join(out_dir, "codex_prompt.md")
        log(f"Writing codex prompt to: {prompt_path}")
        write_text_file(prompt_path, render_codex_prompt(repo, out_dir, tag, prev_tag))

        overview_path = os.path.join(out_dir, "overview.md")
        log(f"Running `codex exec` to generate highlights: {overview_path}")
        run_codex_exec(prompt_path, overview_path)

        print(changelog_md)
        print()
        print(f"Outputs written to: {out_dir}")
        print(f"- {changelog_path}")
        print(f"- {overview_path}")

        release_notes_path = os.path.join(out_dir, "release_notes.md")
        log(f"Writing combined release notes to: {release_notes_path}")
        write_text_file(
            release_notes_path,
            read_text_file(overview_path).rstrip()
            + "\n\n"
            + read_text_file(changelog_path),
        )
        print(f"- {release_notes_path}")

        if output:
            output_path = resolve_copy_destination(output, "release_notes.md")
            log(f"Copying release notes to: {output_path}")
            os.makedirs(os.path.dirname(output_path), exist_ok=True)
            shutil.copyfile(release_notes_path, output_path)
        return 0
    except ReleaseNotesError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print(f"Partial outputs (if any) are in: {out_dir}", file=sys.stderr)
        return 1


def parse_repo(repo: str) -> Repo:
    match = re.fullmatch(r"([^/]+)/([^/]+)", repo)
    if not match:
        raise ReleaseNotesError(f"Invalid repo: {repo}. Expected OWNER/NAME.")
    return Repo(owner=match.group(1), name=match.group(2))


def normalize_release_tag(tag: str) -> str:
    if tag.startswith("rust-v"):
        return tag
    if tag.startswith("v"):
        return f"rust-{tag}"
    if re.fullmatch(r"\d+\.\d+\.\d+", tag):
        return f"rust-v{tag}"
    raise ReleaseNotesError(f"Unrecognized tag format: {tag}")


def derive_prev_tag(tag: str) -> str:
    semver = extract_semver(tag)
    major, minor, patch = semver
    if minor == 0:
        raise ReleaseNotesError(
            f"Cannot derive previous tag from {tag}: minor version is 0."
        )
    return f"rust-v{major}.{minor - 1}.{patch}"


def extract_semver(tag: str) -> tuple[int, int, int]:
    match = re.search(r"v(\d+)\.(\d+)\.(\d+)$", tag)
    if not match:
        raise ReleaseNotesError(f"Could not parse semver from tag: {tag}")
    return (int(match.group(1)), int(match.group(2)), int(match.group(3)))


def run_gh_api(
    endpoint: str, *, method: str = "GET", payload: dict | None = None
) -> dict | list:
    command = [
        "gh",
        "api",
        endpoint,
        "--method",
        method,
        "-H",
        "Accept: application/vnd.github+json",
    ]
    stdin = None
    if payload is not None:
        command.extend(["--input", "-"])
        stdin = json.dumps(payload).encode("utf-8")
    try:
        completed = subprocess.run(
            command,
            input=stdin,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
            timeout=GH_TIMEOUT_SECONDS,
            env={
                **os.environ,
                "GIT_TERMINAL_PROMPT": "0",
                "GH_PROMPT_DISABLED": "1",
            },
        )
    except subprocess.TimeoutExpired as error:
        raise ReleaseNotesError(f"Timed out running: {' '.join(command)}") from error
    except subprocess.CalledProcessError as error:
        stderr = error.stderr.decode("utf-8", errors="replace").strip()
        raise ReleaseNotesError(f"gh api failed for {endpoint}: {stderr}") from error

    stdout = completed.stdout.decode("utf-8")
    if stdout:
        return json.loads(stdout)
    return {}


def run_gh_graphql(query: str) -> dict:
    command = [
        "gh",
        "api",
        "graphql",
        "-f",
        f"query={query}",
        "-H",
        "Accept: application/vnd.github+json",
    ]
    try:
        completed = subprocess.run(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
            timeout=GH_TIMEOUT_SECONDS,
            env={
                **os.environ,
                "GIT_TERMINAL_PROMPT": "0",
                "GH_PROMPT_DISABLED": "1",
            },
        )
    except subprocess.TimeoutExpired as error:
        raise ReleaseNotesError("Timed out running gh api graphql.") from error
    except subprocess.CalledProcessError as error:
        stderr = error.stderr.decode("utf-8", errors="replace").strip()
        raise ReleaseNotesError(f"gh api graphql failed: {stderr}") from error

    stdout = completed.stdout.decode("utf-8")
    data = json.loads(stdout)
    if "errors" in data:
        raise ReleaseNotesError(f"GraphQL errors: {data['errors']}")
    return data


def get_default_branch_head(repo: Repo) -> str:
    info = run_gh_api(f"/repos/{repo.owner}/{repo.name}")
    if not isinstance(info, dict):
        raise ReleaseNotesError(f"Unexpected repo response: {info}")
    default_branch = info.get("default_branch")
    if not isinstance(default_branch, str):
        raise ReleaseNotesError(f"Unexpected default branch in repo response: {info}")
    ref = run_gh_api(f"/repos/{repo.owner}/{repo.name}/git/ref/heads/{default_branch}")
    obj = None
    if isinstance(ref, dict):
        obj = ref.get("object")
    if not isinstance(obj, dict):
        raise ReleaseNotesError(f"Unexpected branch ref response: {ref}")
    sha = obj.get("sha")
    if not isinstance(sha, str):
        raise ReleaseNotesError(f"Unexpected branch head SHA response: {ref}")
    return sha


def get_tag_parent_commit(repo: Repo, tag: str, default_branch_head: str) -> str:
    ref = run_gh_api(f"/repos/{repo.owner}/{repo.name}/git/ref/tags/{tag}")
    obj = ref.get("object")
    if not isinstance(obj, dict):
        raise ReleaseNotesError(f"Unexpected tag ref response for {tag}: {ref}")

    obj_type = obj.get("type")
    obj_sha = obj.get("sha")
    if not isinstance(obj_type, str) or not isinstance(obj_sha, str):
        raise ReleaseNotesError(f"Unexpected tag ref object for {tag}: {obj}")

    if obj_type == "tag":
        annotated_tag = run_gh_api(
            f"/repos/{repo.owner}/{repo.name}/git/tags/{obj_sha}"
        )
        target = annotated_tag.get("object")
        if not isinstance(target, dict) or target.get("type") != "commit":
            raise ReleaseNotesError(
                f"Unexpected annotated tag for {tag}: {annotated_tag}"
            )
        commit_sha = target.get("sha")
        if not isinstance(commit_sha, str):
            raise ReleaseNotesError(
                f"Unexpected annotated tag commit SHA for {tag}: {annotated_tag}"
            )
    elif obj_type == "commit":
        commit_sha = obj_sha
    else:
        raise ReleaseNotesError(f"Unsupported tag object type for {tag}: {obj_type}")

    commit = run_gh_api(f"/repos/{repo.owner}/{repo.name}/commits/{commit_sha}")
    parents = commit.get("parents")
    if not isinstance(parents, list) or not parents:
        raise ReleaseNotesError(f"Unexpected commit parents for tag {tag}: {commit}")

    parent_shas: list[str] = []
    for parent in parents:
        if not isinstance(parent, dict):
            continue
        sha = parent.get("sha")
        if isinstance(sha, str):
            parent_shas.append(sha)
    if not parent_shas:
        raise ReleaseNotesError(f"Unexpected parent SHA list for tag {tag}: {commit}")

    if len(parent_shas) == 1:
        return parent_shas[0]

    for sha in parent_shas:
        compare = run_gh_api(
            f"/repos/{repo.owner}/{repo.name}/compare/{sha}...{default_branch_head}"
        )
        status = None
        if isinstance(compare, dict):
            status = compare.get("status")
        if status in {"behind", "identical"}:
            return sha

    return parent_shas[0]


def compare_commits(repo: Repo, base: str, head: str) -> list[str]:
    shas: list[str] = []

    page = 1
    total_commits: int | None = None
    while True:
        data = run_gh_api(
            f"/repos/{repo.owner}/{repo.name}/compare/{base}...{head}?per_page=100&page={page}"
        )
        if not isinstance(data, dict):
            raise ReleaseNotesError(f"Unexpected compare response: {data}")
        if total_commits is None:
            maybe_total = data.get("total_commits")
            if isinstance(maybe_total, int):
                total_commits = maybe_total

        commits = data.get("commits")
        if not isinstance(commits, list):
            raise ReleaseNotesError(f"Unexpected compare commits response: {data}")

        page_shas = []
        for commit in commits:
            if not isinstance(commit, dict):
                continue
            sha = commit.get("sha")
            if isinstance(sha, str):
                page_shas.append(sha)

        if not page_shas:
            break

        shas.extend(page_shas)
        if total_commits is not None and len(shas) >= total_commits:
            break
        page += 1
        if page > 50:
            raise ReleaseNotesError(
                f"Aborting: compare returned too many pages (base={base}, head={head})."
            )

    if total_commits is not None and len(shas) < total_commits:
        raise ReleaseNotesError(
            f"Only fetched {len(shas)} of {total_commits} commits; compare pagination may have failed."
        )

    return shas


def collect_prs_for_commits(repo: Repo, commit_shas: list[str]) -> list[PullRequest]:
    prs_by_number: dict[int, PullRequest] = {}
    ordered_prs: list[PullRequest] = []

    chunk_size = 25
    for i in range(0, len(commit_shas), chunk_size):
        chunk = commit_shas[i : i + chunk_size]
        query = build_associated_prs_query(repo, chunk)
        data = run_gh_graphql(query)

        repository = None
        if isinstance(data, dict):
            data_root = data.get("data")
            if isinstance(data_root, dict):
                repository = data_root.get("repository")
        if not isinstance(repository, dict):
            raise ReleaseNotesError(f"Unexpected GraphQL response: {data}")

        for idx in range(len(chunk)):
            key = f"c{idx}"
            obj = repository.get(key)
            if not isinstance(obj, dict):
                continue
            associated = obj.get("associatedPullRequests")
            if not isinstance(associated, dict):
                continue
            nodes = associated.get("nodes")
            if not isinstance(nodes, list):
                continue
            for node in nodes:
                if not isinstance(node, dict):
                    continue
                number = node.get("number")
                title = node.get("title")
                author_login = None
                author = node.get("author")
                if isinstance(author, dict):
                    login = author.get("login")
                    if isinstance(login, str):
                        author_login = login
                if not isinstance(number, int) or not isinstance(title, str):
                    continue
                if number in prs_by_number:
                    continue
                pr = PullRequest(number=number, title=title, author_login=author_login)
                prs_by_number[number] = pr
                ordered_prs.append(pr)

    return ordered_prs


def build_associated_prs_query(repo: Repo, commit_shas: list[str]) -> str:
    parts: list[str] = [
        "query{repository(owner:",
        json.dumps(repo.owner),
        ",name:",
        json.dumps(repo.name),
        "){",
    ]
    for i, sha in enumerate(commit_shas):
        parts.extend(
            [
                f"c{i}:object(oid:",
                json.dumps(sha),
                "){...on Commit{associatedPullRequests(first:10){nodes{number title author{login}}}}}",
            ]
        )
    parts.append("}}")
    return "".join(parts)


def render_changelog(
    repo: Repo, prev_tag: str, tag: str, prs: list[PullRequest]
) -> str:
    compare_url = (
        f"https://github.com/{repo.owner}/{repo.name}/compare/{prev_tag}...{tag}"
    )
    lines: list[str] = ["## Changelog", "", f"Full Changelog: {compare_url}", ""]
    for pr in prs:
        author = ""
        if pr.author_login:
            author = f" @{pr.author_login}"
        lines.append(f"- #{pr.number} {pr.title}{author}")
    return "\n".join(lines) + "\n"


def write_text_file(path: str, contents: str) -> None:
    with open(path, "w", encoding="utf-8") as f:
        f.write(contents)


def read_text_file(path: str) -> str:
    with open(path, "r", encoding="utf-8") as f:
        return f.read()


def resolve_copy_destination(copy_to: str, filename: str) -> str:
    if os.path.isdir(copy_to) or copy_to.endswith(os.sep):
        return os.path.join(copy_to, filename)
    return copy_to


async def fetch_and_write_pr_details(
    repo: Repo,
    pr_cache_dir: str,
    pr_out_dir: str,
    prs: list[PullRequest],
    max_concurrency: int,
    comments_limit: int,
) -> list[dict]:
    if max_concurrency <= 0:
        raise ReleaseNotesError("--max-concurrency must be > 0.")
    if comments_limit < 0:
        raise ReleaseNotesError("--comments-limit must be >= 0.")

    details: list[dict] = []
    semaphore = asyncio.Semaphore(max_concurrency)

    async def fetch_one_pr(pr: PullRequest, idx: int) -> dict:
        async with semaphore:
            log(f"PR {idx}/{len(prs)}: #{pr.number}")
            cache_path = os.path.join(pr_cache_dir, f"{pr.number}.json")
            pr_info = read_cached_json(cache_path)
            if not isinstance(pr_info, dict):
                pr_info = await run_gh_api_async(
                    f"/repos/{repo.owner}/{repo.name}/pulls/{pr.number}"
                )
                if not isinstance(pr_info, dict):
                    raise ReleaseNotesError(
                        f"Unexpected PR response for #{pr.number}: {pr_info}"
                    )
                pr_info["files"] = await fetch_pr_files_async(repo, pr.number)
                if comments_limit > 0:
                    pr_info["issue_comments"] = await fetch_issue_comments_async(
                        repo, pr.number, comments_limit
                    )
                write_text_file(
                    cache_path, json.dumps(pr_info, indent=2, sort_keys=True) + "\n"
                )
            else:
                log(f"PR {idx}/{len(prs)}: reusing cached JSON for #{pr.number}")

            out_path = os.path.join(pr_out_dir, f"{pr.number}.json")
            if os.path.abspath(out_path) != os.path.abspath(cache_path):
                shutil.copyfile(cache_path, out_path)
            return summarize_pr_for_prompt(pr_info)

    tasks = [fetch_one_pr(pr, idx) for idx, pr in enumerate(prs, start=1)]
    results = await asyncio.gather(*tasks)
    details.extend(results)
    return details


def fetch_pr_files(repo: Repo, pr_number: int) -> list[dict]:
    files: list[dict] = []
    page = 1
    while True:
        data = run_gh_api(
            f"/repos/{repo.owner}/{repo.name}/pulls/{pr_number}/files?per_page=100&page={page}"
        )
        if not isinstance(data, list):
            raise ReleaseNotesError(
                f"Unexpected PR files response for #{pr_number}: {data}"
            )
        if not data:
            break
        for item in data:
            if isinstance(item, dict):
                files.append(item)
        page += 1
        if page > 20:
            raise ReleaseNotesError(
                f"Aborting: too many file pages for PR #{pr_number}."
            )
    return files


async def fetch_pr_files_async(repo: Repo, pr_number: int) -> list[dict]:
    files: list[dict] = []
    page = 1
    while True:
        data = await run_gh_api_async(
            f"/repos/{repo.owner}/{repo.name}/pulls/{pr_number}/files?per_page=100&page={page}"
        )
        if not isinstance(data, list):
            raise ReleaseNotesError(
                f"Unexpected PR files response for #{pr_number}: {data}"
            )
        if not data:
            break
        for item in data:
            if isinstance(item, dict):
                files.append(slim_pr_file(item))
        page += 1
        if page > 20:
            raise ReleaseNotesError(
                f"Aborting: too many file pages for PR #{pr_number}."
            )
    return files


async def fetch_issue_comments_async(
    repo: Repo, pr_number: int, limit: int
) -> list[dict]:
    comments: list[dict] = []
    page = 1
    while len(comments) < limit:
        per_page = min(100, limit - len(comments))
        data = await run_gh_api_async(
            f"/repos/{repo.owner}/{repo.name}/issues/{pr_number}/comments"
            f"?per_page={per_page}&page={page}&sort=created&direction=desc"
        )
        if not isinstance(data, list):
            raise ReleaseNotesError(
                f"Unexpected issue comments response for #{pr_number}: {data}"
            )
        if not data:
            break
        for item in data:
            if isinstance(item, dict):
                comments.append(slim_issue_comment(item))
                if len(comments) >= limit:
                    break
        page += 1
        if page > 20:
            raise ReleaseNotesError(
                f"Aborting: too many comment pages for PR #{pr_number}."
            )
    return comments


def slim_pr_file(file_info: dict) -> dict:
    return {
        "filename": file_info.get("filename"),
        "status": file_info.get("status"),
        "additions": file_info.get("additions"),
        "deletions": file_info.get("deletions"),
        "changes": file_info.get("changes"),
    }


def slim_issue_comment(comment: dict) -> dict:
    user = comment.get("user")
    login = None
    if isinstance(user, dict):
        maybe_login = user.get("login")
        if isinstance(maybe_login, str):
            login = maybe_login
    return {
        "id": comment.get("id"),
        "created_at": comment.get("created_at"),
        "user": {"login": login},
        "body": comment.get("body"),
    }


def summarize_pr_for_prompt(pr_info: dict) -> dict:
    number = pr_info.get("number")
    title = pr_info.get("title")
    user = pr_info.get("user")
    author_login = None
    if isinstance(user, dict):
        login = user.get("login")
        if isinstance(login, str):
            author_login = login
    labels = pr_info.get("labels")
    label_names: list[str] = []
    if isinstance(labels, list):
        for label in labels:
            if not isinstance(label, dict):
                continue
            name = label.get("name")
            if isinstance(name, str):
                label_names.append(name)

    files = pr_info.get("files")
    filenames: list[str] = []
    if isinstance(files, list):
        for f in files:
            if not isinstance(f, dict):
                continue
            filename = f.get("filename")
            if isinstance(filename, str):
                filenames.append(filename)

    return {
        "number": number,
        "title": title,
        "author": author_login,
        "labels": label_names,
        "files": filenames,
    }


def render_codex_prompt(repo: Repo, out_dir: str, tag: str, prev_tag: str) -> str:
    return f"""You are writing GitHub release notes for {repo.owner}/{repo.name}.
The release tag is: {tag}
The previous release tag is: {prev_tag}

You have access to a directory of PR metadata JSON files:
- Output dir: {out_dir}
- PR index: {out_dir}/prs_index.json
- Per-PR JSON: {out_dir}/prs/<PR_NUMBER>.json

Each per-PR JSON file includes:
- Core PR fields from GitHub's pulls API
- `files`: file metadata WITHOUT patch content (to reduce size)
- `issue_comments`: up to the most recent comments (if present)

Use shell commands to read only the parts you need and keep context small. Examples:
- List PRs: `jq -r '.[] | "#\\(.number) \\(.title)"' {out_dir}/prs_index.json`
- PR summary: `jq '{{number, title, user: .user.login, labels: [.labels[].name]}}' {out_dir}/prs/123.json`
- File list: `jq -r '.files[].filename' {out_dir}/prs/123.json`
- Recent comments (truncated): `jq -r '.issue_comments[0:5][] | "\\(.user.login): \\(.body[:200])"' {out_dir}/prs/123.json`

Write a markdown document with these sections (omit empty sections):
## New Features
## Bug Fixes
## Documentation
## Chores

Rules:
- These sections should be HIGHLIGHTS, not an exhaustive list. Do not mention every PR.
- Prefer 0-6 bullets per section.
- Each bullet should describe a user-facing change in your own words (do not copy PR titles verbatim).
- If a highlight spans multiple PRs, list each referenced number at the end in parentheses, e.g. "(#123, #456)".
- Keep bullets concise and concrete; avoid vague phrasing.
- Output only the markdown sections; no preamble, no conclusion.
"""


def run_codex_exec(prompt_path: str, output_path: str) -> None:
    with open(prompt_path, "rb") as f:
        prompt_bytes = f.read()
    try:
        completed = subprocess.run(
            [
                "codex",
                "exec",
                "-s",
                "read-only",
                "--output-last-message",
                output_path,
                "-",
            ],
            input=prompt_bytes,
            check=True,
            timeout=CODEX_TIMEOUT_SECONDS,
            env={
                **os.environ,
                "CODEX_NO_UPDATE": "1",
                "GIT_TERMINAL_PROMPT": "0",
            },
        )
    except FileNotFoundError as error:
        raise ReleaseNotesError(
            "codex CLI not found on PATH (needed for `codex exec`)."
        ) from error
    except subprocess.TimeoutExpired as error:
        raise ReleaseNotesError("Timed out running `codex exec`.") from error
    except subprocess.CalledProcessError as error:
        raise ReleaseNotesError(
            f"`codex exec` failed with exit code {error.returncode}."
        ) from error
    _ = completed


def read_cached_json(path: str) -> dict | list | None:
    if not os.path.exists(path):
        return None
    try:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f)
    except json.JSONDecodeError:
        return None


async def run_gh_api_async(
    endpoint: str, *, method: str = "GET", payload: dict | None = None
) -> dict | list:
    command: list[str] = [
        "gh",
        "api",
        endpoint,
        "--method",
        method,
        "-H",
        "Accept: application/vnd.github+json",
    ]
    stdin: bytes | None = None
    if payload is not None:
        command.extend(["--input", "-"])
        stdin = json.dumps(payload).encode("utf-8")

    try:
        proc = await asyncio.create_subprocess_exec(
            *command,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            env={
                **os.environ,
                "GIT_TERMINAL_PROMPT": "0",
                "GH_PROMPT_DISABLED": "1",
            },
        )
    except FileNotFoundError as error:
        raise ReleaseNotesError("`gh` not found on PATH.") from error

    try:
        stdout, stderr = await asyncio.wait_for(
            proc.communicate(input=stdin), timeout=GH_TIMEOUT_SECONDS
        )
    except asyncio.TimeoutError as error:
        raise ReleaseNotesError(f"Timed out running: {' '.join(command)}") from error

    if proc.returncode != 0:
        stderr_text = stderr.decode("utf-8", errors="replace").strip()
        raise ReleaseNotesError(f"gh api failed for {endpoint}: {stderr_text}")

    stdout_text = stdout.decode("utf-8")
    if stdout_text:
        return json.loads(stdout_text)
    return {}


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
