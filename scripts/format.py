#!/usr/bin/env python3
"""Format repository sources or check that they are already formatted."""

import argparse
import os
import shlex
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CODEX_RS_ROOT = REPO_ROOT / "codex-rs"
BUILDIFIER = REPO_ROOT / "tools" / "buildifier"


@dataclass(frozen=True)
class Command:
    args: tuple[str, ...]
    cwd: Path = REPO_ROOT
    discard_stderr: bool = False


@dataclass(frozen=True)
class FormatterGroup:
    name: str
    commands: tuple[Command, ...]


@dataclass(frozen=True)
class FormatterResult:
    name: str
    output: str
    returncode: int


def buildifier_formatter_group(*, check: bool) -> FormatterGroup:
    repository_files = subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=REPO_ROOT,
    ).split(b"\0")
    buildifier_files: list[str] = []
    for encoded_path in repository_files:
        if not encoded_path:
            continue
        path = Path(os.fsdecode(encoded_path))
        name = path.name
        if (
            name in {"BUILD", "WORKSPACE", "MODULE.bazel"}
            or name.startswith(("BUILD.", "WORKSPACE."))
            or name.endswith((".BUILD.bazel", ".MODULE.bazel", ".bzl", ".sky"))
            or ".bzl." in name
            or ".sky." in name
        ):
            buildifier_files.append(path.as_posix())
    buildifier_files.sort()

    # Invoke DotSlash explicitly because Windows does not honor shebangs.
    buildifier_args = [
        "dotslash",
        str(BUILDIFIER),
        "-mode=check" if check else "-mode=fix",
        "-lint=off",
        *buildifier_files,
    ]
    return FormatterGroup("Bazel/Starlark", (Command(tuple(buildifier_args)),))


def formatter_groups(*, check: bool) -> tuple[FormatterGroup, ...]:
    just_args = ["just", "--unstable", "--fmt"]
    cargo_args = ["cargo", "fmt", "--", "--config", "imports_granularity=Item"]
    # Each `--project` retains its local dependency and Ruff configuration context.
    sdk_uv_run_args = [
        "uv",
        "run",
        "--frozen",
        "--project",
        "sdk/python",
        "--only-group",
        "format",
    ]
    scripts_uv_run_args = [
        "uv",
        "run",
        "--frozen",
        "--project",
        "scripts",
    ]
    sdk_format_args = [
        *sdk_uv_run_args,
        "ruff",
        "format",
    ]
    scripts_format_args = [
        *scripts_uv_run_args,
        "ruff",
        "format",
    ]

    if check:
        just_args.append("--check")
        cargo_args.append("--check")
        sdk_format_args.append("--check")
        scripts_format_args.append("--check")
        # `ruff check --diff` reports lint-driven rewrites without changing files.
        # It is the check-mode counterpart of `--fix --fix-only`, not a full lint gate.
        sdk_lint_args = ["ruff", "check", "--diff"]
    else:
        # Ruff's lint fixer and formatter are separate passes: the first applies
        # fixable lint rewrites, while the second formats source layout.
        sdk_lint_args = ["ruff", "check", "--fix", "--fix-only"]

    return (
        FormatterGroup("Just", (Command(tuple(just_args)),)),
        FormatterGroup(
            "Rust",
            # Stable rustfmt repeats a nightly-only `imports_granularity` warning
            # for each crate, so suppress that expected stderr noise.
            (Command(tuple(cargo_args), CODEX_RS_ROOT, discard_stderr=True),),
        ),
        buildifier_formatter_group(check=check),
        FormatterGroup(
            "Python SDK",
            (
                Command(
                    (
                        *sdk_uv_run_args,
                        *sdk_lint_args,
                        "sdk/python",
                    )
                ),
                Command((*sdk_format_args, "sdk/python")),
            ),
        ),
        FormatterGroup(
            "Python scripts",
            (
                # The SDK and internal scripts intentionally use separate project
                # roots so uv and Ruff retain each project's configuration context.
                Command((*scripts_format_args, "scripts")),
            ),
        ),
    )


def run_formatter_group(group: FormatterGroup) -> FormatterResult:
    """Run one formatter group sequentially and return its buffered output."""
    output: list[str] = []
    for command in group.commands:
        output.append(f"$ {shlex.join(command.args)}\n")
        try:
            process = subprocess.run(
                command.args,
                cwd=command.cwd,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL
                if command.discard_stderr
                else subprocess.STDOUT,
                text=True,
                check=False,
            )
        except OSError as error:
            output.append(f"{error}\n")
            return FormatterResult(group.name, "".join(output), 1)

        output.append(process.stdout)
        if process.stdout and not process.stdout.endswith("\n"):
            output.append("\n")
        if process.returncode != 0:
            return FormatterResult(group.name, "".join(output), process.returncode)

    return FormatterResult(group.name, "".join(output), 0)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="check formatting without modifying files",
    )
    args = parser.parse_args()
    groups = formatter_groups(check=args.check)

    failures: list[str] = []
    with ThreadPoolExecutor(max_workers=len(groups)) as executor:
        futures = {}
        for group in groups:
            print(f"Starting {group.name} formatter...", flush=True)
            futures[executor.submit(run_formatter_group, group)] = group.name
        for future in as_completed(futures):
            result = future.result()
            print(f"==> {result.name} formatter finished")
            print(result.output, end="")
            if result.returncode != 0:
                failures.append(result.name)

    if failures:
        print(f"Formatting failed: {', '.join(failures)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
