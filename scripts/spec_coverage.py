#!/usr/bin/env python3
"""
Spec coverage report.

Scans the repository for code files (default: *.rs) and reports how many have
adjacent `*.spec.md` documents. Supports a `.specignore` file with glob-style
patterns (similar to .gitignore) to exclude paths from the calculation.
"""

from __future__ import annotations

import argparse
import fnmatch
import os
from pathlib import Path, PurePosixPath
from typing import Iterable


DEFAULT_EXTENSIONS = (".rs",)


def load_ignore_patterns(root: Path) -> list[str]:
    specignore = root / ".specignore"
    if not specignore.exists():
        return []
    patterns: list[str] = []
    for raw_line in specignore.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        # Normalise directory patterns to end with /
        if line.endswith(os.sep):
            line = line.replace(os.sep, "/")
        patterns.append(line)
    return patterns


def is_ignored(rel_path: Path, patterns: Iterable[str]) -> bool:
    if not patterns:
        return False
    posix = PurePosixPath(rel_path).as_posix()
    for pattern in patterns:
        if not pattern:
            continue
        if pattern.endswith("/"):
            prefix = pattern[:-1]
            if posix == prefix or posix.startswith(prefix + "/"):
                return True
        elif fnmatch.fnmatch(posix, pattern):
            return True
    return False


def gather_code_files(root: Path, extensions: tuple[str, ...], patterns: list[str]) -> list[Path]:
    result: list[Path] = []
    for path in root.rglob("*"):
        if path.is_dir():
            continue
        if not path.suffix in extensions:
            continue
        if path.name.endswith(".spec.md") or path.suffix == ".spec":
            continue
        rel = path.relative_to(root)
        if is_ignored(rel, patterns):
            continue
        result.append(path)
    return result


def expected_spec_path(code_path: Path) -> Path:
    return code_path.with_suffix(code_path.suffix + ".spec.md")


def format_percent(numerator: int, denominator: int) -> str:
    if denominator == 0:
        return "N/A"
    return f"{(numerator / denominator) * 100:.2f}%"


def main() -> None:
    parser = argparse.ArgumentParser(description="Report spec coverage for code files.")
    parser.add_argument(
        "--root",
        type=Path,
        default=Path.cwd(),
        help="Root directory to scan (default: current working directory)",
    )
    parser.add_argument(
        "--extensions",
        type=str,
        default=",".join(DEFAULT_EXTENSIONS),
        help="Comma-separated list of file extensions to include (default: .rs)",
    )
    args = parser.parse_args()

    root = args.root.resolve()
    if not root.exists():
        raise SystemExit(f"Root path {root} does not exist")

    extensions = tuple(
        ext if ext.startswith(".") else f".{ext}"
        for ext in (e.strip() for e in args.extensions.split(","))
        if ext.strip()
    )
    ignore_patterns = load_ignore_patterns(root)
    code_files = gather_code_files(root, extensions, ignore_patterns)

    total = len(code_files)
    covered = 0
    missing: list[Path] = []

    for code_file in code_files:
        spec_path = expected_spec_path(code_file)
        if spec_path.exists():
            covered += 1
        else:
            missing.append(code_file.relative_to(root))

    print(f"Spec coverage report for {root}")
    print(f"  Total code files: {total}")
    print(f"  Covered by specs: {covered}")
    print(f"  Coverage: {format_percent(covered, total)}")

    if missing:
        print(f"\nFiles without specs ({len(missing)}):")
        for path in sorted(missing):
            print(f"  {path.as_posix()}")
    else:
        print("\nAll files covered!")


if __name__ == "__main__":
    main()
