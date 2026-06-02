#!/usr/bin/env python3
"""Format repository sources or check that they are already formatted."""

import argparse
import shlex
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CODEX_RS_ROOT = REPO_ROOT / "codex-rs"


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


def formatter_groups(*, check: bool) -> tuple[FormatterGroup, ...]:
    just_args = ["just", "--unstable", "--fmt"]
    cargo_args = ["cargo", "fmt", "--", "--config", "imports_granularity=Item"]
    # Use an unpinned overlay so Ruff is available without syncing project
    # dependencies. Each `--project` still retains its local configuration context.
    sdk_uv_run_args = [
        "uv",
        "run",
        "--frozen",
        "--project",
        "sdk/python",
        "--no-sync",
        "--with",
        "ruff",
    ]
    scripts_uv_run_args = [
        "uv",
        "run",
        "--frozen",
        "--project",
        "scripts",
        "--no-sync",
        "--with",
        "ruff",
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
            (Command(tuple(cargo_args), CODEX_RS_ROOT, discard_stderr=True),),
        ),
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
