#!/usr/bin/env python3

"""Assign Windows Bazel test targets using checked-in duration estimates."""

import argparse
import sys
from pathlib import Path


DEFAULT_DURATION_SECONDS = 1
DURATIONS_PATH = Path(__file__).with_name("windows_bazel_test_durations.tsv")


def read_durations(path: Path) -> dict[str, int]:
    durations = {}
    for line_number, line in enumerate(
        path.read_text(encoding="utf-8").splitlines(), 1
    ):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 2 or not fields[0] or not fields[1].isdecimal():
            raise ValueError(
                f"{path}:{line_number}: expected target<TAB>positive integer"
            )
        target, duration_text = fields
        duration = int(duration_text)
        if duration < 1:
            raise ValueError(f"{path}:{line_number}: duration must be positive")
        if target in durations:
            raise ValueError(f"{path}:{line_number}: duplicate target {target}")
        durations[target] = duration
    return durations


def assign_targets(
    targets: list[str], durations: dict[str, int], shard_count: int
) -> tuple[list[list[str]], list[int]]:
    if shard_count < 1:
        raise ValueError("shard count must be positive")
    if not targets:
        raise ValueError("no Bazel test targets were provided")
    if any(not target.strip() for target in targets):
        raise ValueError("Bazel test target labels must not be empty")
    if len(targets) != len(set(targets)):
        raise ValueError("duplicate Bazel test target labels were provided")

    assignments: list[list[str]] = [[] for _ in range(shard_count)]
    loads = [0] * shard_count
    # Place the longest targets first, breaking ties by label and shard number.
    for target in sorted(
        targets,
        key=lambda target: (-durations.get(target, DEFAULT_DURATION_SECONDS), target),
    ):
        shard = min(range(shard_count), key=lambda shard: (loads[shard], shard))
        assignments[shard].append(target)
        loads[shard] += durations.get(target, DEFAULT_DURATION_SECONDS)
    for assignment in assignments:
        assignment.sort()
    return assignments, loads


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--shard", required=True, type=int)
    parser.add_argument("--shard-count", required=True, type=int)
    parser.add_argument(
        "--durations",
        type=Path,
        default=DURATIONS_PATH,
        help="use this timing file instead of the checked-in default",
    )
    args = parser.parse_args()
    if not 1 <= args.shard <= args.shard_count:
        parser.error("shard must be between 1 and shard count")

    try:
        durations = read_durations(args.durations)
        targets = sys.stdin.read().splitlines()
        assignments, loads = assign_targets(targets, durations, args.shard_count)
    except (OSError, ValueError) as error:
        parser.error(str(error))

    selected_targets = assignments[args.shard - 1]
    if not selected_targets:
        parser.error(
            f"no Bazel test targets selected for shard {args.shard}/{args.shard_count}"
        )

    target_set = set(targets)
    print(
        f"Windows Bazel shards: targets={[len(assignment) for assignment in assignments]}, "
        f"estimated test-seconds={loads}, default weights={len(target_set - durations.keys())}, "
        f"unused weights={len(durations.keys() - target_set)}. "
        f"Selected shard {args.shard}/{args.shard_count}.",
        file=sys.stderr,
    )
    # Keep LF delimiters when native Windows Python feeds Git Bash mapfile.
    sys.stdout.buffer.write(("\n".join(selected_targets) + "\n").encode("utf-8"))


if __name__ == "__main__":
    main()
