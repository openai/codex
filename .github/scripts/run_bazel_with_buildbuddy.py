#!/usr/bin/env python3

import os
import subprocess
import sys
from collections.abc import Mapping
from collections.abc import Sequence


# These configs select remote execution, which is unavailable on CI runners.
REMOTE_EXECUTION_CONFIGS = {
    "--config=ci-linux",
    "--config=ci-macos",
    "--config=ci-v8",
    "--config=ci-windows-cross",
}
# Honor either explicit setting so the wrapper never overrides the caller's
# choice when it supplies the CI default below.
REMOTE_REPO_CONTENTS_CACHE_STARTUP_OPTIONS = {
    "--experimental_remote_repo_contents_cache",
    "--noexperimental_remote_repo_contents_cache",
}


def startup_args(args: Sequence[str], env: Mapping[str, str]) -> list[str]:
    """Return shared startup options that are missing from a Bazel invocation.

    Bazel startup options must precede the command, and changing them restarts
    the server and discards its analysis cache. GitHub Actions invokes Bazel
    through several helpers, so normalize their startup options here while
    preserving any explicit choice made by the caller.
    """
    command_idx = next(
        (idx for idx, arg in enumerate(args) if not arg.startswith("-")),
        len(args),
    )
    configured_startup_args = args[:command_idx]
    injected_args = []

    output_user_root = env.get("BAZEL_OUTPUT_USER_ROOT")
    if output_user_root and not any(
        arg.startswith("--output_user_root=") for arg in configured_startup_args
    ):
        injected_args.append(f"--output_user_root={output_user_root}")

    if env.get("GITHUB_ACTIONS") == "true" and not any(
        arg in REMOTE_REPO_CONTENTS_CACHE_STARTUP_OPTIONS
        for arg in configured_startup_args
    ):
        # Work around Bazel 9 overlay materialization failures seen in CI. This
        # disables only the startup-level repo contents cache.
        injected_args.append("--noexperimental_remote_repo_contents_cache")

    return injected_args


def bazel_args_without_remote_execution(args: Sequence[str]) -> list[str]:
    try:
        separator_idx = args.index("--")
    except ValueError:
        separator_idx = len(args)
    return [
        *(arg for arg in args[:separator_idx] if arg not in REMOTE_EXECUTION_CONFIGS),
        *args[separator_idx:],
    ]


def bazel_args_with_remote_config(
    args: Sequence[str], env: Mapping[str, str]
) -> list[str]:
    command_idx = next(
        (idx for idx, arg in enumerate(args) if not arg.startswith("-")),
        None,
    )
    if command_idx is None:
        raise ValueError("expected a Bazel command")

    configured_args = bazel_args_without_remote_execution(args)
    try:
        separator_idx = configured_args.index("--")
    except ValueError:
        separator_idx = len(configured_args)

    cache_args = [
        f"{option_prefix}{env[env_name]}"
        for env_name, option_prefix in (
            ("BAZEL_REPO_CONTENTS_CACHE", "--repo_contents_cache="),
            ("BAZEL_REPOSITORY_CACHE", "--repository_cache="),
        )
        if env.get(env_name)
        and not any(
            arg.startswith(option_prefix) for arg in configured_args[:separator_idx]
        )
    ]
    return [
        *configured_args[:separator_idx],
        *cache_args,
        *configured_args[separator_idx:],
    ]


def bazel_command(*args: str, env: Mapping[str, str] | None = None) -> list[str]:
    env = os.environ if env is None else env
    bazel = env.get("CODEX_BAZEL_BIN", "bazel")
    return [bazel, *startup_args(args, env), *bazel_args_with_remote_config(args, env)]


def main() -> None:
    command = bazel_command(*sys.argv[1:])
    if os.name == "nt":
        # Windows CRT exec can split arguments containing spaces and lose the
        # eventual child exit status. Wait for Bazel and propagate its status.
        result = subprocess.run(command, check=False)
        raise SystemExit(result.returncode)

    os.execvp(command[0], command)


if __name__ == "__main__":
    main()
