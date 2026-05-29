#!/usr/bin/env python3

from __future__ import annotations

import json
import os
import sys
from collections.abc import Mapping
from collections.abc import Sequence
from pathlib import Path


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


# Only authenticated workflow runs executing trusted upstream code may use the
# OpenAI BuildBuddy host. A pull request event without proof that its head is
# in the upstream repository fails closed to the generic host.
def is_trusted_upstream_run(env: Mapping[str, str]) -> bool:
    if (
        env.get("GITHUB_ACTIONS") != "true"
        or env.get("GITHUB_REPOSITORY") != OPENAI_REPOSITORY
    ):
        return False
    if env.get("GITHUB_EVENT_NAME") != "pull_request":
        return True

    event_path = env.get("GITHUB_EVENT_PATH")
    if not event_path:
        return False
    try:
        event = json.loads(Path(event_path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return False

    try:
        return event["pull_request"]["head"]["repo"]["fork"] is False
    except (KeyError, TypeError):
        return False


def uses_openai_host(env: Mapping[str, str]) -> bool:
    return bool(env.get("BUILDBUDDY_API_KEY")) and is_trusted_upstream_run(env)


def uses_remote_execution(args: Sequence[str]) -> bool:
    try:
        separator_idx = args.index("--")
    except ValueError:
        separator_idx = len(args)
    return any(arg in REMOTE_EXECUTION_CONFIGS for arg in args[:separator_idx])


def remote_config(args: Sequence[str], env: Mapping[str, str]) -> str | None:
    if not env.get("BUILDBUDDY_API_KEY"):
        return None

    config = OPENAI_REMOTE_CONFIG if uses_openai_host(env) else GENERIC_REMOTE_CONFIG
    if uses_remote_execution(args):
        config += "-rbe"
    return config


def bazel_args_with_remote_config(
    args: Sequence[str], env: Mapping[str, str]
) -> list[str]:
    config = remote_config(args, env)
    if config is None:
        # Remote CI configs require BuildBuddy credentials. Removing them
        # preserves the local fallback used for fork pull requests.
        try:
            separator_idx = args.index("--")
        except ValueError:
            separator_idx = len(args)
        return [
            *(
                arg
                for arg in args[:separator_idx]
                if arg not in REMOTE_EXECUTION_CONFIGS
            ),
            *args[separator_idx:],
        ]

    api_key = env["BUILDBUDDY_API_KEY"]
    remote_args = [
        f"--config={config}",
        f"--remote_header=x-buildbuddy-api-key={api_key}",
    ]

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
    if config is None:
        print(
            "BuildBuddy key unavailable; using local Bazel configuration.",
            file=sys.stderr,
        )
    else:
        host_description = (
            "OpenAI tenant" if uses_openai_host(os.environ) else "generic"
        )
        print(
            f"Using {host_description} BuildBuddy configuration: {config}.",
            file=sys.stderr,
        )

    command = bazel_command(*sys.argv[1:])
    os.execvp(command[0], command)


if __name__ == "__main__":
    main()
