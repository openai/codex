#!/usr/bin/env python3

"""Run Bazel in CI with platform configuration and useful failure output."""

import os
import re
import subprocess
import sys
from collections import deque
from collections.abc import Mapping
from collections.abc import Sequence
from dataclasses import dataclass
from io import StringIO
from pathlib import Path
from tempfile import NamedTemporaryFile
from typing import TextIO

sys.path.insert(0, str(Path(__file__).resolve().parent))
from run_bazel_with_buildbuddy import buildbuddy_wrapper_command


USAGE = (
    "Usage: run_bazel_ci.py [--print-failed-test-logs] "
    "[--print-failed-action-summary] [--remote-download-toplevel] "
    "[--windows-msvc-host-platform] [--windows-cross-compile] "
    "-- <bazel args> -- <targets>"
)
WINDOWS_ACTION_ENV_VARS = (
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "UCRTVersion",
    "UniversalCRTSdkDir",
    "VCINSTALLDIR",
    "VCToolsInstallDir",
    "WindowsLibPath",
    "WindowsSdkBinPath",
    "WindowsSdkDir",
    "WindowsSDKLibVersion",
    "WindowsSDKVersion",
)
INFO_POST_CONFIG_PREFIXES = (
    "--host_platform=",
    "--repo_contents_cache=",
    "--repository_cache=",
)
ANSI_ESCAPE_RE = re.compile(r"\x1b\[[0-9;]*m")
LOG_PREFIX_RE = re.compile(r"^.*\t[^\t]*\t[0-9TZ:._-]+ ")
FAILED_TARGET_RES = (
    re.compile(r"^FAIL: (//[^ ]+)"),
    re.compile(r"^ERROR: .* Testing (//[^ ]+) failed:"),
)
REPORTED_TEST_LOG_RE = re.compile(r" \(see (.*[\\/]test\.log)\)")


@dataclass(frozen=True)
class Options:
    print_failed_test_logs: bool = False
    print_failed_action_summary: bool = False
    remote_download_toplevel: bool = False
    windows_msvc_host_platform: bool = False
    windows_cross_compile: bool = False


@dataclass(frozen=True)
class Invocation:
    command: list[str]
    child_env: dict[str, str]
    ci_config: str
    post_config_args: list[str]
    remote_enabled: bool


def parse_args(args: Sequence[str]) -> tuple[Options, list[str], list[str]]:
    option_names = {
        "--print-failed-test-logs": "print_failed_test_logs",
        "--print-failed-action-summary": "print_failed_action_summary",
        "--remote-download-toplevel": "remote_download_toplevel",
        "--windows-msvc-host-platform": "windows_msvc_host_platform",
        "--windows-cross-compile": "windows_cross_compile",
    }
    enabled = {name: False for name in option_names.values()}
    index = 0
    while index < len(args) and args[index] != "--":
        arg = args[index]
        if arg not in option_names:
            raise ValueError(f"Unknown option: {arg}")
        enabled[option_names[arg]] = True
        index += 1
    if index == len(args):
        raise ValueError(USAGE)

    remaining = list(args[index + 1 :])
    try:
        separator = remaining.index("--")
    except ValueError as exc:
        raise ValueError("Expected Bazel args and targets separated by --") from exc
    bazel_args = remaining[:separator]
    targets = remaining[separator + 1 :]
    if not bazel_args or not targets:
        raise ValueError("Expected Bazel args and targets separated by --")
    return Options(**enabled), bazel_args, targets


def ci_config(options: Options, env: Mapping[str, str]) -> str:
    if env.get("RUNNER_OS") == "macOS":
        return "ci-macos"
    if env.get("RUNNER_OS") == "Windows":
        if options.windows_cross_compile and env.get("BUILDBUDDY_API_KEY"):
            return "ci-windows-cross"
        return "ci-windows"
    return "ci-linux"


def post_config_args(
    options: Options,
    bazel_args: Sequence[str],
    env: Mapping[str, str],
    *,
    pid: int,
) -> list[str]:
    runner_os = env.get("RUNNER_OS")
    is_windows = runner_os == "Windows"
    has_buildbuddy_key = bool(env.get("BUILDBUDDY_API_KEY"))
    args: list[str] = []

    use_msvc_host = options.windows_msvc_host_platform or (
        options.windows_cross_compile and not has_buildbuddy_key
    )
    if is_windows and use_msvc_host and not any(
        arg.startswith("--host_platform=") for arg in bazel_args
    ):
        args.append("--host_platform=//:local_windows_msvc")
    if options.remote_download_toplevel:
        args.append("--remote_download_toplevel")
    if is_windows and options.windows_cross_compile and has_buildbuddy_key:
        args.extend(("--host_platform=//:rbe", "--shell_executable=/bin/bash"))
    if is_windows and options.windows_cross_compile and not has_buildbuddy_key:
        args.append("--jobs=8")
    if repo_contents_cache := env.get("BAZEL_REPO_CONTENTS_CACHE"):
        args.append(f"--repo_contents_cache={repo_contents_cache}")
    if repository_cache := env.get("BAZEL_REPOSITORY_CACHE"):
        args.append(f"--repository_cache={repository_cache}")
    if execution_log_dir := env.get("CODEX_BAZEL_EXECUTION_LOG_COMPACT_DIR"):
        job = env.get("GITHUB_JOB", "local")
        args.append(
            f"--execution_log_compact_file={execution_log_dir}/"
            f"execution-log-{bazel_args[0]}-{job}-{pid}.zst"
        )

    if not is_windows:
        return args

    windows_path = env.get("CODEX_BAZEL_WINDOWS_PATH")
    if not windows_path:
        raise ValueError("CODEX_BAZEL_WINDOWS_PATH must be set for Windows Bazel CI.")

    pass_windows_build_env = not (
        options.windows_cross_compile and has_buildbuddy_key
    )
    if pass_windows_build_env:
        for name in WINDOWS_ACTION_ENV_VARS:
            if value := env.get(name):
                args.extend((f"--action_env={name}", f"--host_action_env={name}"))
        args.extend(
            (
                f"--action_env=PATH={windows_path}",
                f"--host_action_env=PATH={windows_path}",
            )
        )
    else:
        args.extend(
            (
                "--action_env=PATH=/usr/bin:/bin",
                "--host_action_env=PATH=/usr/bin:/bin",
            )
        )
    args.append(f"--test_env=PATH={windows_path}")
    return args


def build_invocation(
    options: Options,
    bazel_args: Sequence[str],
    targets: Sequence[str],
    env: Mapping[str, str],
    *,
    pid: int,
) -> Invocation:
    config = ci_config(options, env)
    post_args = post_config_args(options, bazel_args, env, pid=pid)
    run_args = list(bazel_args)
    remote_enabled = bool(env.get("BUILDBUDDY_API_KEY"))
    if remote_enabled:
        run_args.append(f"--config={config}")
    run_args.extend(post_args)

    startup_args = []
    if output_user_root := env.get("BAZEL_OUTPUT_USER_ROOT"):
        startup_args.append(f"--output_user_root={output_user_root}")
    command = buildbuddy_wrapper_command(
        *startup_args,
        "--noexperimental_remote_repo_contents_cache",
        *run_args,
        "--",
        *targets,
    )
    child_env = dict(env)
    if env.get("RUNNER_OS") == "Windows":
        child_env["MSYS2_ARG_CONV_EXCL"] = "*"
    return Invocation(command, child_env, config, post_args, remote_enabled)


def clean_log_line(line: str) -> str:
    return LOG_PREFIX_RE.sub("", ANSI_ESCAPE_RE.sub("", line))


def is_diagnostic(line: str) -> bool:
    stripped = line.lstrip()
    return (
        bool(re.match(r"^(error(\[[^]]+\])?:|warning:|note:|help:)", line))
        or stripped.startswith("-->")
        or bool(re.match(r"^[0-9]+[ ]+\|", stripped))
        or stripped.startswith("|")
        or stripped.startswith("= note:")
        or stripped.startswith("= help:")
        or bool(re.match(r"^\^[\s^~-]*$", stripped))
        or line.startswith("For more information")
        or line.startswith("error: aborting")
    )


def action_failure_summary_from_lines(lines: TextIO) -> str | None:
    summary: list[str] = []
    fallback_summary: deque[str] = deque(maxlen=50)
    in_failure = False
    seen_diagnostic = False
    for raw_line in lines:
        raw_line = raw_line.rstrip("\r\n")
        if raw_line.startswith(("ERROR: ", "FAILED: ")):
            fallback_summary.append(raw_line)
        line = clean_log_line(raw_line)
        if line.startswith("ERROR: ") and " failed:" in line:
            if summary:
                summary.append("")
            summary.append(line)
            in_failure = True
            seen_diagnostic = False
        elif in_failure and is_diagnostic(line):
            summary.append(line)
            seen_diagnostic = True
        elif in_failure and seen_diagnostic and not line:
            summary.append("")
        elif in_failure and seen_diagnostic:
            in_failure = False
            seen_diagnostic = False

    if not summary:
        summary = list(fallback_summary)
    result = "\n".join(summary).rstrip()
    return result or None


def action_failure_summary(console_output: str) -> str | None:
    return action_failure_summary_from_lines(StringIO(console_output))


def failed_test_targets_from_lines(lines: TextIO) -> list[str]:
    targets = set()
    for line in lines:
        for pattern in FAILED_TARGET_RES:
            if match := pattern.match(line):
                targets.add(match.group(1))
                break
    return sorted(targets)


def failed_test_targets(console_output: str) -> list[str]:
    return failed_test_targets_from_lines(StringIO(console_output))


def reported_test_log_from_lines(lines: TextIO, target: str) -> Path | None:
    prefix = f"FAIL: {target} "
    for line in lines:
        if line.startswith(prefix) and (match := REPORTED_TEST_LOG_RE.search(line)):
            return Path(match.group(1).replace("\\", "/"))
    return None


def reported_test_log(console_output: str, target: str) -> Path | None:
    return reported_test_log_from_lines(StringIO(console_output), target)


def test_log_path(console_output: str, testlogs_dir: Path, target: str) -> Path:
    if path := reported_test_log(console_output, target):
        return path
    relative = target.removeprefix("//").replace(":", "/", 1)
    return testlogs_dir / relative / "test.log"


def bazel_testlogs_dir(invocation: Invocation) -> Path:
    info_args = ["info"]
    if invocation.remote_enabled:
        info_args.append(f"--config={invocation.ci_config}")
    info_args.extend(
        arg
        for arg in invocation.post_config_args
        if arg.startswith(INFO_POST_CONFIG_PREFIXES)
    )
    command = buildbuddy_wrapper_command(
        "--noexperimental_remote_repo_contents_cache",
        *info_args,
        "bazel-testlogs",
    )
    result = subprocess.run(
        command,
        env=invocation.child_env,
        check=False,
        capture_output=True,
        text=True,
    )
    return Path(result.stdout.strip()) if result.returncode == 0 else Path("bazel-testlogs")


def print_action_failure_summary(console_log: Path) -> None:
    with console_log.open(encoding="utf-8", errors="replace") as lines:
        summary = action_failure_summary_from_lines(lines)
    if summary is None:
        print("No Bazel action failures were found in the captured console output.")
        return
    if os.environ.get("GITHUB_ACTIONS") == "true":
        escaped = summary.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
        print(f"::error title=Bazel failed action diagnostics::{escaped}")
    print("\nBazel failed action diagnostics:")
    print("--------------------------------")
    print(summary)
    print("--------------------------------")


def print_test_log_tails(console_log: Path, invocation: Invocation) -> None:
    with console_log.open(encoding="utf-8", errors="replace") as lines:
        targets = failed_test_targets_from_lines(lines)
    if not targets:
        print("No failed Bazel test targets were found in console output.")
        return
    testlogs_dir = bazel_testlogs_dir(invocation)
    for target in targets:
        with console_log.open(encoding="utf-8", errors="replace") as lines:
            path = reported_test_log_from_lines(lines, target)
        if path is None:
            relative = target.removeprefix("//").replace(":", "/", 1)
            path = testlogs_dir / relative / "test.log"
        print(f"::group::Bazel test log tail for {target}")
        if path.is_file():
            with path.open(encoding="utf-8", errors="replace") as lines:
                print("".join(deque(lines, maxlen=200)), end="")
        else:
            print(f"Missing test log: {path}")
        print("::endgroup::")


def run_and_tee(invocation: Invocation) -> tuple[int, Path]:
    with NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        prefix="bazel-console-",
        suffix=".log",
        delete=False,
    ) as console_log:
        process = subprocess.Popen(
            invocation.command,
            env=invocation.child_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            encoding="utf-8",
            errors="replace",
            text=True,
        )
        assert process.stdout is not None
        for line in process.stdout:
            print(line, end="", flush=True)
            console_log.write(line)
        return process.wait(), Path(console_log.name)


def main(argv: Sequence[str] | None = None, env: Mapping[str, str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    env = os.environ if env is None else env
    try:
        options, bazel_args, targets = parse_args(argv)
        invocation = build_invocation(options, bazel_args, targets, env, pid=os.getpid())
    except ValueError as exc:
        print(exc, file=sys.stderr)
        return 1

    if invocation.remote_enabled:
        print("BuildBuddy API key is available; using remote Bazel configuration.")
    else:
        print("BuildBuddy API key is not available; using local Bazel configuration.")
    status, console_log = run_and_tee(invocation)
    try:
        if status != 0:
            if options.print_failed_action_summary:
                print_action_failure_summary(console_log)
            if options.print_failed_test_logs:
                print_test_log_tails(console_log, invocation)
        return status
    finally:
        console_log.unlink(missing_ok=True)


if __name__ == "__main__":
    sys.exit(main())
