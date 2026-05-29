#!/usr/bin/env python3

from __future__ import annotations

import os
import sys
from collections.abc import Mapping
from collections.abc import Sequence


OPENAI_REPOSITORY = "openai/codex"
# Remote configurations select cache/BES/download endpoints. Their -rbe forms
# also select the matching remote executor endpoint.
GENERIC_REMOTE_CONFIG = "buildbuddy-generic"
OPENAI_REMOTE_CONFIG = "buildbuddy-openai"
# These configurations select remote build execution. The wrapper applies an
# RBE endpoint configuration when one is already part of the Bazel invocation.
REMOTE_EXECUTION_CONFIGS = {
    "--config=remote",
    "--config=ci-linux",
    "--config=ci-macos",
    "--config=ci-v8",
    "--config=ci-windows-cross",
}


def uses_openai_host(env: Mapping[str, str]) -> bool:
    has_key = bool(env.get("BUILDBUDDY_API_KEY"))
    opts_in = env.get("CODEX_BAZEL_USE_OPENAI_BUILDBUDDY_HOST") == "true"
    if not has_key or not opts_in:
        return False

    repository = env.get("GITHUB_REPOSITORY")
    if not repository:
        raise RuntimeError(
            "GITHUB_REPOSITORY must be set to select the OpenAI BuildBuddy host"
        )

    return repository == OPENAI_REPOSITORY


def uses_remote_execution(args: Sequence[str]) -> bool:
    return any(arg in REMOTE_EXECUTION_CONFIGS for arg in args)


def remote_config(args: Sequence[str], env: Mapping[str, str]) -> str:
    config = OPENAI_REMOTE_CONFIG if uses_openai_host(env) else GENERIC_REMOTE_CONFIG
    if uses_remote_execution(args):
        config += "-rbe"
    return config


def bazel_args_with_remote_config(
    args: Sequence[str], env: Mapping[str, str]
) -> list[str]:
    remote_args = [f"--config={remote_config(args, env)}"]
    if api_key := env.get("BUILDBUDDY_API_KEY"):
        remote_args.append(f"--remote_header=x-buildbuddy-api-key={api_key}")

    try:
        separator_idx = args.index("--")
    except ValueError:
        # No target separator is present, so command options can be appended.
        return [*args, *remote_args]

    # Insert command options before a Bazel `--` separator so targets or
    # `bazel run` arguments after the separator do not absorb the remote config.
    return [*args[:separator_idx], *remote_args, *args[separator_idx:]]


def bazel_command(*args: str, env: Mapping[str, str] | None = None) -> list[str]:
    env = os.environ if env is None else env
    bazel = env.get("CODEX_BAZEL_BIN", "bazel")
    return [bazel, *bazel_args_with_remote_config(args, env)]


def main() -> None:
    config = remote_config(sys.argv[1:], os.environ)
    host_description = "OpenAI tenant" if uses_openai_host(os.environ) else "generic"
    print(
        f"Using {host_description} BuildBuddy configuration: {config}.",
        file=sys.stderr,
    )

    command = bazel_command(*sys.argv[1:])
    os.execvp(command[0], command)


if __name__ == "__main__":
    main()
