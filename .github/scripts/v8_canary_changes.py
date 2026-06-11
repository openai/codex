#!/usr/bin/env python3

import argparse
import subprocess
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WINDOWS_SOURCE_BUILD_PATHS = {
    ".github/scripts/rusty_v8_bazel.py",
    ".github/scripts/v8_canary_changes.py",
    ".github/workflows/rusty-v8-release.yml",
    ".github/workflows/v8-canary.yml",
}


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
    return (
        force
        or base_v8_version != head_v8_version
        or not changed_files.isdisjoint(WINDOWS_SOURCE_BUILD_PATHS)
    )


def git_output(*args: str) -> bytes:
    return subprocess.check_output(["git", *args], cwd=ROOT)


def v8_version_at_revision(revision: str) -> str:
    return resolved_v8_version(git_output("show", f"{revision}:codex-rs/Cargo.lock"))


def changed_files(base: str, head: str) -> set[str]:
    output = git_output("diff", "--name-only", "--no-renames", base, head)
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
        required = True
        reason = "manual workflow dispatch"
    elif not args.base or not args.head:
        raise SystemExit("--base and --head are required unless --force is set")
    else:
        files = changed_files(args.base, args.head)
        base_version = v8_version_at_revision(args.base)
        head_version = v8_version_at_revision(args.head)
        required = windows_source_required(files, base_version, head_version)
        if base_version != head_version:
            reason = f"v8 version changed from {base_version} to {head_version}"
        else:
            matched_paths = sorted(files & WINDOWS_SOURCE_BUILD_PATHS)
            reason = (
                ", ".join(matched_paths) if matched_paths else "no relevant changes"
            )

    print(f"windows_source_required={str(required).lower()}")
    print(f"windows_source_reason={reason}")


if __name__ == "__main__":
    main()
