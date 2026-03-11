#!/usr/bin/env python3

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


DEFAULT_MAX_BYTES = 500 * 1024


@dataclass(frozen=True)
class BinaryBlob:
    path: str
    size_bytes: int
    is_allowlisted: bool


def run_git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout


def load_allowlist(path: Path) -> set[str]:
    allowlist: set[str] = set()
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if line:
            allowlist.add(line)
    return allowlist


def get_changed_paths(base: str, head: str) -> list[str]:
    output = run_git(
        "diff",
        "--name-only",
        "--diff-filter=AM",
        "--no-renames",
        "-z",
        base,
        head,
    )
    return [path for path in output.split("\0") if path]


def is_binary_change(base: str, head: str, path: str) -> bool:
    output = run_git(
        "diff",
        "--numstat",
        "--diff-filter=AM",
        "--no-renames",
        base,
        head,
        "--",
        path,
    ).strip()
    if not output:
        return False

    added, deleted, _ = output.split("\t", 2)
    return added == "-" and deleted == "-"


def blob_size(commit: str, path: str) -> int:
    return int(run_git("cat-file", "-s", f"{commit}:{path}").strip())


def collect_changed_binary_blobs(
    base: str,
    head: str,
    allowlist: set[str],
) -> list[BinaryBlob]:
    binary_blobs: list[BinaryBlob] = []
    for path in get_changed_paths(base, head):
        if is_binary_change(base, head, path):
            binary_blobs.append(
                BinaryBlob(
                    path=path,
                    size_bytes=blob_size(head, path),
                    is_allowlisted=path in allowlist,
                )
            )
    return binary_blobs


def format_kib(size_bytes: int) -> str:
    return f"{size_bytes / 1024:.1f} KiB"


def write_step_summary(
    max_bytes: int,
    binary_blobs: list[BinaryBlob],
    violations: list[BinaryBlob],
) -> None:
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not summary_path:
        return

    lines = [
        "## Binary Blob Size Policy",
        "",
        f"Default max: `{max_bytes}` bytes ({format_kib(max_bytes)})",
        f"Changed binary files: `{len(binary_blobs)}`",
        f"Violations: `{len(violations)}`",
        "",
    ]

    if binary_blobs:
        lines.extend(
            [
                "| Path | Size | Status |",
                "| --- | ---: | --- |",
            ]
        )
        for blob in binary_blobs:
            status = "allowlisted" if blob.is_allowlisted else "ok"
            if blob in violations:
                status = "blocked"
            lines.append(
                f"| `{blob.path}` | `{blob.size_bytes}` bytes ({format_kib(blob.size_bytes)}) | {status} |"
            )
    else:
        lines.append("No changed binary files were detected.")

    lines.append("")
    Path(summary_path).write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Fail if changed binary blobs exceed the configured size budget."
    )
    parser.add_argument("--base", required=True, help="Base git revision to diff against.")
    parser.add_argument("--head", required=True, help="Head git revision to inspect.")
    parser.add_argument(
        "--max-bytes",
        type=int,
        default=DEFAULT_MAX_BYTES,
        help=f"Maximum allowed binary blob size in bytes. Default: {DEFAULT_MAX_BYTES}.",
    )
    parser.add_argument(
        "--allowlist",
        type=Path,
        required=True,
        help="Path to the newline-delimited allowlist file.",
    )
    args = parser.parse_args()

    allowlist = load_allowlist(args.allowlist)
    binary_blobs = collect_changed_binary_blobs(args.base, args.head, allowlist)
    violations = [
        blob
        for blob in binary_blobs
        if blob.size_bytes > args.max_bytes and not blob.is_allowlisted
    ]

    write_step_summary(args.max_bytes, binary_blobs, violations)

    if not binary_blobs:
        print("No changed binary files were detected.")
        return 0

    print(
        f"Checked {len(binary_blobs)} changed binary file(s) against the {args.max_bytes}-byte limit."
    )
    for blob in binary_blobs:
        status = "allowlisted" if blob.is_allowlisted else "ok"
        if blob in violations:
            status = "blocked"
        print(
            f"- {blob.path}: {blob.size_bytes} bytes ({format_kib(blob.size_bytes)}) [{status}]"
        )

    if violations:
        print("\nBinary file(s) exceed the configured limit:")
        for blob in violations:
            print(
                f"- {blob.path}: {blob.size_bytes} bytes > {args.max_bytes} bytes"
            )
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
