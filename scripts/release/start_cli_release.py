#!/usr/bin/env python3

import argparse
import base64
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from urllib.parse import quote

REPO = "openai/codex"
RELEASE_WORKFLOW = "rust-release.yml"
MAIN_BRANCH = "main"
CARGO_TOML_PATH = "codex-rs/Cargo.toml"
RELEASE_NOTES_TIMEOUT_SECONDS = 20 * 60
RELEASE_RUN_TIMEOUT_SECONDS = 60


@dataclass(frozen=True)
class ReleasePlan:
    version: str
    base_commit: str
    previous_release_tag: str | None = None
    expected_cargo_version: str | None = None
    cargo_version_context: str | None = None
    dry_run_message: str | None = None


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Publish a tagged Codex release.")
    parser.add_argument(
        "-n",
        "--dry-run",
        action="store_true",
        help="Print the version that would be used and exit before making changes.",
    )
    parser.add_argument(
        "--promote-alpha",
        metavar="VERSION",
        help="Promote an existing alpha tag (e.g., 0.56.0-alpha.5) by using its merge-base with main as the base commit.",
    )
    parser.add_argument(
        "--release-line",
        metavar="MAJOR.MINOR",
        help="Stable release line to patch when using --publish-release-from-branch, e.g. 0.142.",
    )

    group = parser.add_mutually_exclusive_group()
    group.add_argument(
        "--publish-alpha",
        action="store_true",
        help="Publish the next alpha release for the upcoming minor version.",
    )
    group.add_argument(
        "--publish-release",
        action="store_true",
        help="Publish the next stable release by bumping the minor version.",
    )
    group.add_argument(
        "--publish-release-from-branch",
        metavar="BRANCH",
        help="Publish the next stable patch release from the tip of a release branch.",
    )
    group.add_argument(
        "--publish-alpha-hotfix-from-branch",
        metavar="BRANCH",
        help=(
            "Publish an alpha hotfix from a branch named "
            "release/MAJOR.MINOR.PATCH-alpha.ALPHA.HOTFIX."
        ),
    )
    parser.add_argument(
        "-E",
        "--emergency-version-override",
        metavar="version",
        help="Publish a specific version because tag was created for the previous release but it never succeeded. Value should be semver (optional leading `v`), e.g., `0.43.0-alpha.9`.",
    )

    args = parser.parse_args(argv[1:])
    release_modes = [
        args.publish_alpha,
        args.publish_release,
        args.publish_release_from_branch is not None,
        args.publish_alpha_hotfix_from_branch is not None,
        args.emergency_version_override is not None,
        args.promote_alpha is not None,
    ]
    if not any(release_modes):
        parser.error(
            "Must specify --publish-alpha, --publish-release, "
            "--publish-release-from-branch, --publish-alpha-hotfix-from-branch, "
            "--promote-alpha, or "
            "--emergency-version-override."
        )
    if sum(release_modes) != 1:
        parser.error(
            "Specify exactly one of --publish-alpha, --publish-release, "
            "--publish-release-from-branch, --publish-alpha-hotfix-from-branch, "
            "--promote-alpha, or "
            "--emergency-version-override."
        )
    if args.release_line and args.publish_release_from_branch is None:
        parser.error(
            "--release-line can only be used with --publish-release-from-branch."
        )
    return args


def strip_leading_v(version: str) -> str:
    """Conservative check so that something like `v0.56.0-alpha.5` gets
    normalized to `0.56.0-alpha.5`."""
    if version.startswith("v") and len(version) > 1 and version[1].isdigit():
        return version[1:]
    return version


def main(argv: list[str]) -> int:
    args = parse_args(argv)

    ensure_release_not_in_progress()

    try:
        plan = build_release_plan(args)
        print(f"Publishing version {plan.version}")
        print(f"Base commit: {plan.base_commit}")
        print("Fetching Cargo.toml...")
        current_contents = fetch_file_contents(plan.base_commit)
        if plan.expected_cargo_version:
            require_cargo_version(
                current_contents,
                plan.expected_cargo_version,
                plan.cargo_version_context or plan.base_commit,
            )
        if args.dry_run:
            if plan.dry_run_message:
                print(plan.dry_run_message)
            return 0

        print("Fetching commit tree...")
        base_tree = get_commit_tree(plan.base_commit)
        print(f"Base tree: {base_tree}")
        print("Updating version...")
        updated_contents = replace_version(current_contents, plan.version)
        print("Creating blob...")
        blob_sha = create_blob(updated_contents)
        print(f"Blob SHA: {blob_sha}")
        print("Creating tree...")
        tree_sha = create_tree(base_tree, blob_sha)
        print(f"Tree SHA: {tree_sha}")
        print("Creating commit...")
        commit_message = derive_commit_message(plan)
        commit_sha = create_commit(commit_message, tree_sha, plan.base_commit)
        print(f"Commit SHA: {commit_sha}")
        print("Creating tag...")
        tag_sha = create_tag(plan.version, commit_sha)
        print(f"Tag SHA: {tag_sha}")
        print("Creating tag ref...")
        create_tag_ref(plan.version, tag_sha)
        print("Waiting for release workflow run...")
        release_run_url = wait_for_release_run_url(plan.version, commit_sha)
        if release_run_url:
            print(f"Release workflow run: {release_run_url}")
        else:
            print(
                f"WARNING: Release tag created, but its workflow run did not appear within {RELEASE_RUN_TIMEOUT_SECONDS} seconds. Check https://github.com/{REPO}/actions/workflows/{RELEASE_WORKFLOW}",
                file=sys.stderr,
            )
        print("Done.")
    except ReleaseError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


def build_release_plan(args: argparse.Namespace) -> ReleasePlan:
    promote_alpha = strip_leading_v(args.promote_alpha) if args.promote_alpha else None
    if promote_alpha:
        version = derive_release_version_from_alpha(promote_alpha)
        base_commit = get_promote_alpha_base_commit(promote_alpha)
        return ReleasePlan(
            version=version,
            base_commit=base_commit,
            dry_run_message=(
                f"Would publish version {version} using base commit {base_commit} "
                f"derived from rust-v{promote_alpha}."
            ),
        )

    if args.publish_release_from_branch is not None:
        return build_branch_release_plan(
            args.publish_release_from_branch, args.release_line
        )

    if args.publish_alpha_hotfix_from_branch is not None:
        return build_alpha_hotfix_release_plan(args.publish_alpha_hotfix_from_branch)

    if args.emergency_version_override:
        version = strip_leading_v(args.emergency_version_override)
    else:
        version = determine_version(args)

    print("Fetching branch head...")
    base_commit = get_branch_head(MAIN_BRANCH)
    return ReleasePlan(
        version=version,
        base_commit=base_commit,
        dry_run_message=f"Would publish version {version} from {MAIN_BRANCH} at {base_commit}.",
    )


def build_alpha_hotfix_release_plan(branch_arg: str) -> ReleasePlan:
    branch = normalize_branch_name(branch_arg)
    match = re.fullmatch(
        r"release/(\d+)\.(\d+)\.(\d+)-alpha\.(\d+)\.(\d+)",
        branch,
    )
    if match is None:
        raise ReleaseError(
            "Alpha hotfix branches must be named release/MAJOR.MINOR.PATCH-alpha.ALPHA.HOTFIX."
        )

    major, minor, patch, alpha, hotfix = (int(part) for part in match.groups())
    if hotfix == 0:
        raise ReleaseError("Alpha hotfix numbers must start at 1.")

    base_alpha_version = f"{major}.{minor}.{patch}-alpha.{alpha}"
    version = f"{base_alpha_version}.{hotfix}"
    if branch != f"release/{version}":
        raise ReleaseError(
            f"Alpha hotfix branch must use canonical name release/{version}."
        )
    base_alpha_release_tag = f"rust-v{base_alpha_version}"
    base_alpha_content_commit = get_only_parent_sha(
        get_tag_commit_sha(base_alpha_release_tag),
        base_alpha_release_tag,
    )
    previous_version = (
        base_alpha_version if hotfix == 1 else f"{base_alpha_version}.{hotfix - 1}"
    )
    previous_release_tag = f"rust-v{previous_version}"
    previous_content_commit = (
        base_alpha_content_commit
        if hotfix == 1
        else get_only_parent_sha(
            get_tag_commit_sha(previous_release_tag),
            previous_release_tag,
        )
    )

    print(f"Fetching branch head for {branch}...")
    base_commit = get_branch_head(branch)
    require_commit_descends_from(
        base_commit,
        previous_content_commit,
        branch,
        previous_release_tag,
    )
    branch_point = get_merge_base_with_main(base_commit)
    if branch_point != base_alpha_content_commit:
        raise ReleaseError(
            f"Branch {branch} must be based on the content commit for "
            f"{base_alpha_release_tag} ({base_alpha_content_commit}), not {branch_point}."
        )
    require_tag_does_not_exist(f"rust-v{version}")

    return ReleasePlan(
        version=version,
        base_commit=base_commit,
        previous_release_tag=previous_release_tag,
        expected_cargo_version="0.0.0",
        cargo_version_context=f"branch {branch}",
        dry_run_message=(
            f"Would publish version {version} from branch {branch} at {base_commit}, "
            f"which continues {previous_release_tag} from the content commit for "
            f"{base_alpha_release_tag}."
        ),
    )


def build_branch_release_plan(
    branch_arg: str, release_line_arg: str | None
) -> ReleasePlan:
    branch = normalize_branch_name(branch_arg)
    if branch == MAIN_BRANCH:
        raise ReleaseError(
            "Use --publish-release for main releases; --publish-release-from-branch "
            "requires a non-main branch."
        )
    if re.fullmatch(r"release/[^/]+", branch) is None:
        raise ReleaseError("Stable release branches must be named release/<name>.")

    release_line = parse_release_line(release_line_arg) if release_line_arg else None
    if release_line is None:
        latest_version = get_latest_release_version()
        major, minor, _patch = parse_semver(latest_version)
        release_line = (major, minor)

    previous_version = get_latest_stable_version_for_line(release_line)
    major, minor, patch = previous_version
    version = format_version(major, minor, patch + 1)
    previous_release_tag = f"rust-v{format_version(*previous_version)}"
    warn_if_branch_version_conflicts(branch, version)

    print(f"Fetching branch head for {branch}...")
    base_commit = get_branch_head(branch)
    return ReleasePlan(
        version=version,
        base_commit=base_commit,
        previous_release_tag=previous_release_tag,
        expected_cargo_version="0.0.0",
        cargo_version_context=f"branch {branch}",
        dry_run_message=(
            f"Would publish version {version} from branch {branch} at {base_commit} "
            f"using previous release {previous_release_tag}."
        ),
    )


class ReleaseError(RuntimeError):
    pass


def run_gh_api(
    endpoint: str, *, method: str = "GET", payload: dict | None = None
) -> dict | list:
    print(f"Running gh api {method} {endpoint}")
    command = [
        "gh",
        "api",
        endpoint,
        "--method",
        method,
        "-H",
        "Accept: application/vnd.github+json",
    ]
    json_payload = None
    if payload is not None:
        json_payload = json.dumps(payload)
        print(f"Payload: {json_payload}")
        command.extend(["-H", "Content-Type: application/json", "--input", "-"])
    result = subprocess.run(command, text=True, capture_output=True, input=json_payload)
    if result.returncode != 0:
        message = result.stderr.strip() or result.stdout.strip() or "gh api call failed"
        raise ReleaseError(message)
    try:
        return json.loads(result.stdout or "{}")
    except json.JSONDecodeError as error:
        raise ReleaseError("Failed to parse response from gh api.") from error


def ensure_release_not_in_progress() -> None:
    """Fail fast if a release workflow is already running or queued."""

    statuses = ("in_progress", "queued")
    runs: list[dict] = []
    for status in statuses:
        response = run_gh_api(
            f"/repos/{REPO}/actions/workflows/{RELEASE_WORKFLOW}/runs?per_page=50&status={status}"
        )
        runs.extend(response.get("workflow_runs", []))

    active_runs = [
        run
        for run in runs
        if run.get("status") in statuses and is_real_release_run(run)
    ]
    if not active_runs:
        return

    seen_ids: set[int] = set()
    urls: list[str] = []
    for run in active_runs:
        run_id = run.get("id")
        if run_id in seen_ids:
            continue
        seen_ids.add(run_id)
        urls.append(run.get("html_url", str(run_id)))

    raise ReleaseError(
        "Release workflow already running or queued; wait or cancel it before publishing: "
        + ", ".join(urls)
    )


def is_real_release_run(run: dict) -> bool:
    """Only treat tag-push rust-release runs as real releases."""

    if run.get("event") != "push":
        return False

    head_branch = run.get("head_branch") or ""
    if not isinstance(head_branch, str):
        return False

    # Tag pushes report the tag name as head_branch; accept refs/tags/rust-v* or rust-v*.
    return bool(re.fullmatch(r"(refs/tags/)?rust-v.+", head_branch))


def get_branch_head(branch: str) -> str:
    encoded_branch = quote(branch, safe="/")
    response = run_gh_api(f"/repos/{REPO}/git/ref/heads/{encoded_branch}")
    try:
        return response["object"]["sha"]
    except KeyError as error:
        raise ReleaseError(f"Unable to determine branch head for {branch}.") from error


def get_promote_alpha_base_commit(alpha_version: str) -> str:
    tag_name = f"rust-v{alpha_version}"
    tag_commit_sha = get_tag_commit_sha(tag_name)
    return get_merge_base_with_main(tag_commit_sha)


def get_tag_commit_sha(tag_name: str) -> str:
    response = run_gh_api(f"/repos/{REPO}/git/refs/tags/{tag_name}")
    try:
        sha = response["object"]["sha"]
        obj_type = response["object"]["type"]
    except KeyError as error:
        raise ReleaseError(f"Unable to resolve tag {tag_name}.") from error
    while obj_type == "tag":
        tag_response = run_gh_api(f"/repos/{REPO}/git/tags/{sha}")
        try:
            sha = tag_response["object"]["sha"]
            obj_type = tag_response["object"]["type"]
        except KeyError as error:
            raise ReleaseError(
                f"Unable to resolve annotated tag {tag_name}."
            ) from error
    if obj_type != "commit":
        raise ReleaseError(f"Tag {tag_name} does not reference a commit.")
    return sha


def get_only_parent_sha(commit_sha: str, context: str) -> str:
    response = run_gh_api(f"/repos/{REPO}/git/commits/{commit_sha}")
    parents = response.get("parents")
    if not isinstance(parents, list) or len(parents) != 1:
        raise ReleaseError(
            f"Expected {context} to reference a commit with exactly one parent."
        )
    parent_sha = parents[0].get("sha")
    if not isinstance(parent_sha, str):
        raise ReleaseError(f"Commit response for {context} is missing its parent SHA.")
    return parent_sha


def require_commit_descends_from(
    commit_sha: str,
    ancestor_sha: str,
    branch: str,
    previous_release_tag: str,
) -> None:
    response = run_gh_api(f"/repos/{REPO}/compare/{ancestor_sha}...{commit_sha}")
    merge_base = response.get("merge_base_commit", {}).get("sha")
    if merge_base != ancestor_sha:
        raise ReleaseError(
            f"Branch {branch} must descend from the content commit for "
            f"{previous_release_tag} ({ancestor_sha})."
        )


def require_tag_does_not_exist(tag_name: str) -> None:
    response = run_gh_api(f"/repos/{REPO}/git/matching-refs/tags/{tag_name}")
    if not isinstance(response, list):
        raise ReleaseError(f"Unexpected response when checking for tag {tag_name}.")
    if any(item.get("ref") == f"refs/tags/{tag_name}" for item in response):
        raise ReleaseError(f"Tag {tag_name} already exists.")


def get_merge_base_with_main(commit_sha: str) -> str:
    response = run_gh_api(f"/repos/{REPO}/compare/main...{commit_sha}")
    try:
        return response["merge_base_commit"]["sha"]
    except KeyError as error:
        raise ReleaseError("Unable to determine merge base with main.") from error


def get_commit_tree(commit_sha: str) -> str:
    response = run_gh_api(f"/repos/{REPO}/git/commits/{commit_sha}")
    try:
        return response["tree"]["sha"]
    except KeyError as error:
        raise ReleaseError("Commit response missing tree SHA.") from error


def fetch_file_contents(ref_sha: str) -> str:
    response = run_gh_api(f"/repos/{REPO}/contents/{CARGO_TOML_PATH}?ref={ref_sha}")
    try:
        encoded_content = response["content"].replace("\n", "")
        encoding = response.get("encoding", "")
    except KeyError as error:
        raise ReleaseError("Failed to fetch Cargo.toml contents.") from error

    if encoding != "base64":
        raise ReleaseError(f"Unexpected Cargo.toml encoding: {encoding}")

    try:
        return base64.b64decode(encoded_content).decode("utf-8")
    except (ValueError, UnicodeDecodeError) as error:
        raise ReleaseError("Failed to decode Cargo.toml contents.") from error


def require_cargo_version(contents: str, expected_version: str, context: str) -> None:
    current_version = get_cargo_version(contents)
    if current_version == expected_version:
        return

    raise ReleaseError(
        f"Expected {CARGO_TOML_PATH} version to be {expected_version} at {context}, "
        f"found {current_version}. Branch releases should be based on content commits, "
        "not release version-bump tag commits."
    )


def get_cargo_version(contents: str) -> str:
    matches = re.findall(r'^version = "([^"]+)"', contents, flags=re.MULTILINE)
    if len(matches) != 1:
        raise ReleaseError("Unable to determine version in Cargo.toml.")
    return matches[0]


def replace_version(contents: str, version: str) -> str:
    updated, matches = re.subn(
        r'^version = "[^"]+"',
        f'version = "{version}"',
        contents,
        count=1,
        flags=re.MULTILINE,
    )
    if matches != 1:
        raise ReleaseError("Unable to update version in Cargo.toml.")
    return updated


def create_blob(content: str) -> str:
    response = run_gh_api(
        f"/repos/{REPO}/git/blobs",
        method="POST",
        payload={"content": content, "encoding": "utf-8"},
    )
    try:
        return response["sha"]
    except KeyError as error:
        raise ReleaseError("Blob creation response missing SHA.") from error


def create_tree(base_tree_sha: str, blob_sha: str) -> str:
    response = run_gh_api(
        f"/repos/{REPO}/git/trees",
        method="POST",
        payload={
            "base_tree": base_tree_sha,
            "tree": [
                {
                    "path": CARGO_TOML_PATH,
                    "mode": "100644",
                    "type": "blob",
                    "sha": blob_sha,
                }
            ],
        },
    )
    try:
        return response["sha"]
    except KeyError as error:
        raise ReleaseError("Tree creation response missing SHA.") from error


def create_commit(message: str, tree_sha: str, parent_sha: str) -> str:
    response = run_gh_api(
        f"/repos/{REPO}/git/commits",
        method="POST",
        payload={
            "message": message,
            "tree": tree_sha,
            "parents": [parent_sha],
        },
    )
    try:
        return response["sha"]
    except KeyError as error:
        raise ReleaseError("Commit creation response missing SHA.") from error


def derive_commit_message(plan: ReleasePlan) -> str:
    if is_alpha_version(plan.version):
        return f"Release {plan.version}"

    print("Generating release notes...")
    body = generate_release_notes_body(
        plan.version,
        plan.base_commit,
        previous_release_tag=plan.previous_release_tag,
    )
    if body.endswith("\n"):
        return body
    return f"{body}\n"


def is_alpha_version(version: str) -> bool:
    return "-alpha." in version


def generate_release_notes_body(
    version: str,
    head_sha: str,
    *,
    previous_release_tag: str | None = None,
) -> str:
    scripts_dir = os.path.dirname(os.path.abspath(__file__))
    script_path = os.path.join(scripts_dir, "generate_release_notes.py")

    with tempfile.TemporaryDirectory(prefix="codex-release-notes-body-") as tmp_dir:
        output_path = os.path.join(tmp_dir, "release_notes.md")

        tag_name = f"rust-v{version}"
        command = [
            sys.executable,
            script_path,
            "--head-sha",
            head_sha,
            "--repo",
            REPO,
            "--output",
            output_path,
        ]
        if previous_release_tag:
            command.extend(["--prev-tag", previous_release_tag])
        command.append(tag_name)
        try:
            subprocess.run(
                command,
                check=True,
                timeout=RELEASE_NOTES_TIMEOUT_SECONDS,
            )
        except subprocess.TimeoutExpired as error:
            raise ReleaseError(
                f"Timed out generating release notes (>{RELEASE_NOTES_TIMEOUT_SECONDS}s)."
            ) from error
        except subprocess.CalledProcessError as error:
            raise ReleaseError(
                f"Failed generating release notes (exit code {error.returncode})."
            ) from error

        try:
            with open(output_path, "r", encoding="utf-8") as f:
                return f.read()
        except OSError as error:
            raise ReleaseError(
                f"Failed reading generated release notes at {output_path}."
            ) from error


def create_tag(version: str, commit_sha: str) -> str:
    tag_name = f"rust-v{version}"
    response = run_gh_api(
        f"/repos/{REPO}/git/tags",
        method="POST",
        payload={
            "tag": tag_name,
            "message": f"Release {version}",
            "object": commit_sha,
            "type": "commit",
        },
    )
    try:
        return response["sha"]
    except KeyError as error:
        raise ReleaseError("Tag creation response missing SHA.") from error


def create_tag_ref(version: str, tag_sha: str) -> None:
    tag_ref = f"refs/tags/rust-v{version}"
    run_gh_api(
        f"/repos/{REPO}/git/refs",
        method="POST",
        payload={"ref": tag_ref, "sha": tag_sha},
    )


def wait_for_release_run_url(version: str, commit_sha: str) -> str | None:
    tag_name = f"rust-v{version}"
    deadline = time.monotonic() + RELEASE_RUN_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        response = run_gh_api(
            f"/repos/{REPO}/actions/workflows/{RELEASE_WORKFLOW}/runs?event=push&head_sha={commit_sha}&per_page=10"
        )
        for run in response.get("workflow_runs", []):
            if run.get("head_branch") in (tag_name, f"refs/tags/{tag_name}"):
                url = run.get("html_url")
                if isinstance(url, str):
                    return url
        time.sleep(1)
    return None


def determine_version(args: argparse.Namespace) -> str:
    latest_version = get_latest_release_version()
    # When determining the next version after the current release,
    # we should always increment the minor version, but reset the
    # patch to zero. In practice, `patch` should only be non-zero if
    # --emergency-version-override was used.
    major, minor, _patch = parse_semver(latest_version)
    next_minor_version = format_version(major, minor + 1, 0)

    if args.publish_release:
        return next_minor_version

    alpha_prefix = f"{next_minor_version}-alpha."
    releases = list_releases()
    highest_alpha = 0
    found_alpha = False
    for release in releases:
        tag = release.get("tag_name", "")
        candidate = strip_tag_prefix(tag)
        if candidate and candidate.startswith(alpha_prefix):
            suffix = candidate[len(alpha_prefix) :]
            try:
                alpha_number = int(suffix)
            except ValueError:
                continue
            highest_alpha = max(highest_alpha, alpha_number)
            found_alpha = True

    if found_alpha:
        return f"{alpha_prefix}{highest_alpha + 1}"
    return f"{alpha_prefix}1"


def get_latest_stable_version_for_line(
    release_line: tuple[int, int],
) -> tuple[int, int, int]:
    major, minor = release_line
    response = run_gh_api(
        f"/repos/{REPO}/git/matching-refs/tags/rust-v{major}.{minor}."
    )
    if not isinstance(response, list):
        raise ReleaseError(
            f"Unexpected response when listing tags for {major}.{minor}."
        )

    versions: list[tuple[int, int, int]] = []
    for item in response:
        if not isinstance(item, dict):
            continue
        ref = item.get("ref")
        if not isinstance(ref, str):
            continue
        version = strip_tag_prefix(ref.removeprefix("refs/tags/"))
        if not version:
            continue
        try:
            candidate = parse_semver(version)
        except ReleaseError:
            continue
        if candidate[0] == major and candidate[1] == minor:
            versions.append(candidate)

    if not versions:
        raise ReleaseError(
            f"No stable releases found for release line {major}.{minor}."
        )
    return max(versions)


def get_latest_release_version() -> str:
    response = run_gh_api(f"/repos/{REPO}/releases/latest")
    tag = response.get("tag_name")
    version = strip_tag_prefix(tag)
    if not version:
        raise ReleaseError("Latest release tag has unexpected format.")
    return version


def list_releases() -> list[dict]:
    response = run_gh_api(f"/repos/{REPO}/releases?per_page=100")
    if not isinstance(response, list):
        raise ReleaseError("Unexpected response when listing releases.")
    return response


def strip_tag_prefix(tag: str | None) -> str | None:
    if not tag:
        return None
    prefix = "rust-v"
    if not tag.startswith(prefix):
        return None
    return tag[len(prefix) :]


def normalize_branch_name(branch: str) -> str:
    branch = branch.removeprefix("refs/heads/")
    branch = branch.removeprefix("heads/")
    if not branch:
        raise ReleaseError("Branch name must not be empty.")
    return branch


def parse_release_line(release_line: str) -> tuple[int, int]:
    match = re.fullmatch(r"v?(\d+)\.(\d+)", release_line)
    if not match:
        raise ReleaseError(
            f"Unexpected release line format: {release_line}. Expected MAJOR.MINOR."
        )
    return int(match.group(1)), int(match.group(2))


def parse_semver(version: str) -> tuple[int, int, int]:
    parts = version.split(".")
    if len(parts) != 3:
        raise ReleaseError(f"Unexpected version format: {version}")
    try:
        return int(parts[0]), int(parts[1]), int(parts[2])
    except ValueError as error:
        raise ReleaseError(f"Version components must be integers: {version}") from error


def format_version(major: int, minor: int, patch: int) -> str:
    return f"{major}.{minor}.{patch}"


def derive_release_version_from_alpha(alpha_version: str) -> str:
    match = re.match(r"^(\d+)\.(\d+)\.(\d+)-alpha\.(\d+)$", alpha_version)
    if match is None:
        raise ReleaseError(f"Unexpected alpha version format: {alpha_version}")
    return f"{match.group(1)}.{match.group(2)}.{match.group(3)}"


def warn_if_branch_version_conflicts(branch: str, version: str) -> None:
    target_major, target_minor, target_patch = parse_semver(version)
    for match in re.finditer(r"(?:rust-)?v?(\d+\.\d+(?:\.\d+)?)", branch):
        parts = tuple(int(part) for part in match.group(1).split("."))
        if len(parts) == 2 and parts != (target_major, target_minor):
            print(
                f"WARNING: branch name {branch} appears to reference {parts[0]}.{parts[1]}, "
                f"but the target release is {target_major}.{target_minor}.{target_patch}.",
                file=sys.stderr,
            )
            return
        if len(parts) == 3 and parts != (target_major, target_minor, target_patch):
            print(
                f"WARNING: branch name {branch} appears to reference "
                f"{parts[0]}.{parts[1]}.{parts[2]}, but the target release is "
                f"{target_major}.{target_minor}.{target_patch}.",
                file=sys.stderr,
            )
            return


if __name__ == "__main__":
    sys.exit(main(sys.argv))
