#!/usr/bin/env python3

"""Decide which V8 canary work is needed for a commit range.

The workflow deliberately has no trigger-level path filters because it is both
directly triggered for pull requests and called by postmerge-ci. Keeping the
patterns here gives those entrypoints one source of truth; unrelated events
still run metadata but skip the expensive build matrices.
"""

import argparse
import subprocess
import tomllib
from fnmatch import fnmatchcase
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
# These patterns replace the old pull_request/push path filters. Include parent
# workflow changes because they can alter whether the canary is invoked.
CANARY_PATH_PATTERNS = {
    ".bazelrc",
    ".github/actions/setup-bazel-ci/**",
    ".github/scripts/run_bazel_with_buildbuddy.py",
    ".github/scripts/rusty_v8_bazel.py",
    ".github/scripts/rusty_v8_module_bazel.py",
    ".github/scripts/v8_canary_changes.py",
    ".github/workflows/postmerge-ci.yml",
    ".github/workflows/rusty-v8-release.yml",
    ".github/workflows/v8-canary.yml",
    "MODULE.bazel",
    "MODULE.bazel.lock",
    "codex-rs/Cargo.toml",
    "patches/BUILD.bazel",
    "patches/llvm_*.patch",
    "patches/rules_cc_*.patch",
    "patches/v8_*.patch",
    "third_party/v8/**",
}
# Windows source builds are a narrower, more expensive subset of the canary.
# A V8 version change also requires them even when no path below changed.
WINDOWS_SOURCE_BUILD_PATHS = {
    ".github/scripts/rusty_v8_bazel.py",
    ".github/scripts/rusty_v8_module_bazel.py",
    ".github/scripts/v8_canary_changes.py",
    ".github/workflows/rusty-v8-release.yml",
    ".github/workflows/v8-canary.yml",
}


def matching_canary_paths(changed_files: set[str]) -> set[str]:
    """Return changed paths that require the general V8 build matrix."""
    return {
        path
        for path in changed_files
        if any(fnmatchcase(path, pattern) for pattern in CANARY_PATH_PATTERNS)
    }


def canary_required(
    changed_files: set[str],
    base_v8_version: str,
    head_v8_version: str,
    *,
    force: bool = False,
) -> bool:
    """Return whether the general V8 build matrix should run."""
    return (
        force
        or base_v8_version != head_v8_version
        or bool(matching_canary_paths(changed_files))
    )


def resolved_v8_version(cargo_lock: bytes) -> str:
    versions = sorted(
        {
            package["version"]
            for package in tomllib.loads(cargo_lock.decode())["package"]
            if package["name"] == "v8"
        }
    )
    if len(versions) != 1:
        raise ValueError(f"expected exactly one resolved v8 version, found: {versions}")
    return versions[0]


def windows_source_required(
    changed_files: set[str],
    base_v8_version: str,
    head_v8_version: str,
    *,
    force: bool = False,
) -> bool:
    """Return whether Windows must rebuild rusty_v8 from source."""
    return (
        force
        or base_v8_version != head_v8_version
        or not changed_files.isdisjoint(WINDOWS_SOURCE_BUILD_PATHS)
    )


def git_output(*args: str, root: Path = ROOT) -> bytes:
    return subprocess.check_output(["git", *args], cwd=root)


def v8_version_at_revision(revision: str, *, root: Path = ROOT) -> str:
    return resolved_v8_version(
        git_output("show", f"{revision}:codex-rs/Cargo.lock", root=root)
    )


def merge_base(base: str, head: str, *, root: Path = ROOT) -> str:
    return git_output("merge-base", base, head, root=root).decode().strip()


def changed_files(base: str, head: str, *, root: Path = ROOT) -> set[str]:
    # Three-dot diff gives PRs merge-base semantics while remaining equivalent
    # to before/after for ordinary linear pushes to main.
    output = git_output(
        "diff",
        "--name-only",
        "--no-renames",
        f"{base}...{head}",
        root=root,
    )
    return set(output.decode().splitlines())


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base")
    parser.add_argument("--head")
    parser.add_argument("--force", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.force:
        # workflow_dispatch has no comparison range, and callers use it as a
        # manual retry path, so it intentionally runs every variant.
        canary = True
        canary_reason = "manual workflow dispatch"
        windows_source = True
        windows_source_reason = "manual workflow dispatch"
    elif not args.base or not args.head:
        raise SystemExit("--base and --head are required unless --force is set")
    else:
        files = changed_files(args.base, args.head)
        base_version = v8_version_at_revision(merge_base(args.base, args.head))
        head_version = v8_version_at_revision(args.head)

        matched_canary_paths = sorted(matching_canary_paths(files))
        canary = canary_required(files, base_version, head_version)
        windows_source = windows_source_required(files, base_version, head_version)
        if base_version != head_version:
            canary_reason = (
                f"v8 version changed from {base_version} to {head_version}"
            )
            windows_source_reason = canary_reason
        else:
            canary_reason = (
                ", ".join(matched_canary_paths)
                if matched_canary_paths
                else "no relevant changes"
            )
            matched_windows_paths = sorted(files & WINDOWS_SOURCE_BUILD_PATHS)
            windows_source_reason = (
                ", ".join(matched_windows_paths)
                if matched_windows_paths
                else "no relevant changes"
            )

    print(f"canary_required={str(canary).lower()}")
    print(f"canary_reason={canary_reason}")
    print(f"windows_source_required={str(windows_source).lower()}")
    print(f"windows_source_reason={windows_source_reason}")


if __name__ == "__main__":
    main()
