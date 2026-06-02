#!/usr/bin/env python3

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
# These CI configurations require remote build execution. The wrapper supplies
# an RBE configuration, which also includes the common `remote` settings.
REMOTE_EXECUTION_CONFIGS = {
    "--config=ci-linux",
    "--config=ci-macos",
    "--config=ci-v8",
    "--config=ci-windows-cross",
}
QUERY_COMMANDS = {"query", "cquery", "aquery"}


# Only authenticated workflow runs executing trusted upstream code may use the
# OpenAI BuildBuddy host. A pull request event without proof that its head is
# in the upstream repository fails closed to the generic host.
def is_trusted_upstream_run(env: Mapping[str, str]) -> bool:
    # `GITHUB_REPOSITORY` is easy to set locally. Requiring GitHub's workflow
    # marker prevents a local command from opting itself into the OpenAI host.
    if (
        env.get("GITHUB_ACTIONS") != "true"
        or env.get("GITHUB_REPOSITORY") != OPENAI_REPOSITORY
    ):
        return False
    # Non-PR workflow runs in `openai/codex` execute upstream refs, so they are
    # trusted. Fork code reaches these workflows only through pull requests.
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


def bazel_args_without_remote_execution(args: Sequence[str]) -> list[str]:
    # Remote CI configs require BuildBuddy credentials. Removing them preserves
    # the local fallback used for fork pull requests.
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
    config = remote_config(args, env)
    if config is None:
        return bazel_args_without_remote_execution(args)

    # `remote_config()` returns a configuration only when this key is present.
    api_key = env["BUILDBUDDY_API_KEY"]
    remote_args = [
        f"--config={config}",
        f"--remote_header=x-buildbuddy-api-key={api_key}",
    ]

    try:
        insertion_idx = args.index("--")
    except ValueError:
        if any(command in args for command in QUERY_COMMANDS):
            # Query commands accept one trailing expression; keep
            # wrapper-added options in front of it without adding a separator.
            insertion_idx = len(args) - 1
        else:
            # No target separator or query expression is present, so command
            # options can be appended.
            insertion_idx = len(args)

    # In both forms, keep the wrapper-added options out of positional payloads:
    # arguments to `bazel run`, or the final query expression.
    return [*args[:insertion_idx], *remote_args, *args[insertion_idx:]]


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
    # Replace the wrapper so Bazel receives signals directly and supplies the
    # command exit status; a subprocess parent would have no remaining work.
    os.execvp(command[0], command)


if __name__ == "__main__":
    main()
