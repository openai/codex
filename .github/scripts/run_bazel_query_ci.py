#!/usr/bin/env python3

"""Run target-discovery Bazel queries with CI cache and server settings."""

from __future__ import annotations

import os
import subprocess
import sys
from collections.abc import Mapping
from collections.abc import Sequence
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from run_bazel_with_buildbuddy import bazel_command


USAGE = "Usage: run_bazel_query_ci.py [<bazel query args>...] -- <query expression>"


def query_command(args: Sequence[str], env: Mapping[str, str]) -> list[str]:
    if len(args) < 2 or args[-2] != "--":
        raise ValueError(USAGE)

    query_args = ["query"]
    if repo_contents_cache := env.get("BAZEL_REPO_CONTENTS_CACHE"):
        query_args.append(f"--repo_contents_cache={repo_contents_cache}")
    if repository_cache := env.get("BAZEL_REPOSITORY_CACHE"):
        query_args.append(f"--repository_cache={repository_cache}")
    query_args.extend(args[:-2])
    query_args.append(args[-1])

    return bazel_command(
        *query_args,
        env=env,
        enable_remote_config=False,
    )


def main(argv: Sequence[str] | None = None, env: Mapping[str, str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    env = os.environ if env is None else env
    try:
        command = query_command(argv, env)
    except ValueError as exc:
        print(exc, file=sys.stderr)
        return 1

    child_env = dict(env)
    if env.get("RUNNER_OS") == "Windows":
        child_env["MSYS2_ARG_CONV_EXCL"] = "*"
    return subprocess.run(command, env=child_env, check=False).returncode


if __name__ == "__main__":
    sys.exit(main())
