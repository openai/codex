#!/usr/bin/env python3
"""
Run codex exec end-to-end with and without the non_login_shell_heuristic feature.

This script repeatedly runs:
- codex exec with the heuristic disabled
- codex exec with the heuristic enabled

for one or more prompts and reports per-run wall times, per-prompt summaries,
and combined summary statistics.

Runs can be parallelized (concurrency > 1) to speed up sampling, but note that
parallel rollouts may contend for local/remote resources and slightly skew
latency compared to strictly serial runs.

Usage:
  python scripts/bench_codex_exec_non_login.py \
    --iterations 5 \
    --prompt "Read and summarize codex_berry" \
    --prompt "Explain the architecture" \
    --workdir /path/to/repo \
    --model gpt-5.1-codex-max \
    --reasoning-effort high \
    --concurrency 2 \
    --codex-bin /path/to/codex \
    --skip-feature-toggle  # when benchmarking an older codex binary

Notes:
- Runs will incur network/LLM variance; prefer N >= 5–10.
- Requires the `codex` binary (local path supported) and valid credentials.
- Results are printed in seconds; failures are logged and included in the output.
"""

import argparse
import asyncio
import json
import math
import statistics
import sys
import time
from dataclasses import dataclass
from typing import List
from typing import Optional


@dataclass
class RunResult:
    duration: float
    exit_code: int
    stderr: str
    command_time: float
    command_count: int
    mcp_call_count: int
    usage: Optional["Usage"]


@dataclass
class Usage:
    input_tokens: int
    cached_input_tokens: int
    output_tokens: int


async def run_once(
    base_cmd: List[str],
    prompt: str,
    feature_enabled: bool,
    toggle_feature: bool,
    sem: asyncio.Semaphore,
) -> RunResult:
    cmd = list(base_cmd)
    if toggle_feature:
        cmd.extend(
            [
                "-c",
                f"features.non_login_shell_heuristic={'true' if feature_enabled else 'false'}",
            ]
        )
    cmd.append(prompt)
    async with sem:
        start = time.perf_counter()
        proc = await asyncio.create_subprocess_exec(
            *cmd, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE
        )

        async def read_stdout() -> tuple[float, int, int, Optional[Usage]]:
            command_start_times: dict[str, float] = {}
            command_time = 0.0
            command_count = 0
            mcp_call_count = 0
            usage: Optional[Usage] = None
            buffer = b""

            def handle_line(raw_line: bytes) -> None:
                nonlocal command_time, command_count, mcp_call_count, usage
                ts = time.perf_counter()
                line = raw_line.decode(errors="replace").strip()
                if not line:
                    return
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    return

                event_type = event.get("type")
                if event_type == "turn.completed" and (usage_info := event.get("usage")):
                    usage = Usage(
                        input_tokens=usage_info.get("input_tokens", 0),
                        cached_input_tokens=usage_info.get("cached_input_tokens", 0),
                        output_tokens=usage_info.get("output_tokens", 0),
                    )

                if not event_type or not event_type.startswith("item."):
                    return

                item = event.get("item") or {}
                item_type = item.get("type")
                item_id = item.get("id")

                if item_type == "command_execution":
                    if event_type == "item.started" and item_id:
                        command_start_times[item_id] = ts
                        command_count += 1
                    elif event_type == "item.completed" and item_id:
                        if start_ts := command_start_times.pop(item_id, None):
                            command_time += ts - start_ts
                elif item_type == "mcp_tool_call":
                    if event_type == "item.started":
                        mcp_call_count += 1

            assert proc.stdout is not None
            try:
                while True:
                    chunk = await proc.stdout.read(4096)
                    if not chunk:
                        break
                    buffer += chunk
                    lines = buffer.split(b"\n")
                    buffer = lines.pop() if lines else b""
                    for raw_line in lines:
                        handle_line(raw_line)
            except asyncio.CancelledError:
                # Allow cancellation to propagate but keep what we have so far.
                raise
            finally:
                if buffer:
                    handle_line(buffer)

            return command_time, command_count, mcp_call_count, usage

        stdout_task = asyncio.create_task(read_stdout())
        stderr_task = (
            asyncio.create_task(proc.stderr.read()) if proc.stderr is not None else None
        )

        await proc.wait()
        stderr_bytes = await stderr_task if stderr_task is not None else b""
        command_time, command_count, mcp_call_count, usage = await stdout_task

        duration = time.perf_counter() - start
        stderr_text = stderr_bytes.decode(errors="replace").strip() if stderr_bytes else ""
        return RunResult(
            duration=duration,
            exit_code=proc.returncode,
            stderr=stderr_text,
            command_time=command_time,
            command_count=command_count,
            mcp_call_count=mcp_call_count,
            usage=usage,
        )


async def run_case(
    label: str,
    base_cmd: List[str],
    prompt: str,
    iterations: int,
    feature_enabled: bool,
    toggle_feature: bool,
    concurrency: int,
) -> tuple[
    list[float], list[float], list[float], list[int], list[int], list[Usage], int
]:
    sem = asyncio.Semaphore(concurrency)
    tasks = [
        asyncio.create_task(
            run_once(base_cmd, prompt, feature_enabled, toggle_feature, sem)
        )
        for _ in range(iterations)
    ]
    durations: List[float] = []
    command_times: List[float] = []
    command_times_per_cmd: List[float] = []
    command_counts: List[int] = []
    mcp_call_counts: List[int] = []
    usages: List[Usage] = []
    failures = 0
    for idx, task in enumerate(asyncio.as_completed(tasks), start=1):
        result = await task
        durations.append(result.duration)
        command_times.append(result.command_time)
        if result.command_count > 0:
            command_times_per_cmd.append(result.command_time / result.command_count)
        command_counts.append(result.command_count)
        mcp_call_counts.append(result.mcp_call_count)
        if result.usage:
            usages.append(result.usage)
        status = "ok" if result.exit_code == 0 else f"fail ({result.exit_code})"
        cmd_time_str = f" cmd_time={result.command_time:.3f}s cmds={result.command_count}"
        mcp_str = f" mcp_calls={result.mcp_call_count}" if result.mcp_call_count else ""
        print(
            f"[{label}] run {idx}/{iterations}: {result.duration:.3f}s"
            f" [{status}]{cmd_time_str}{mcp_str}"
        )
        if result.exit_code != 0:
            failures += 1
            if result.stderr:
                print(f"    stderr: {result.stderr}", file=sys.stderr)
    return (
        durations,
        command_times,
        command_times_per_cmd,
        command_counts,
        mcp_call_counts,
        usages,
        failures,
    )


def summarize(label: str, durations: List[float]) -> None:
    if not durations:
        print(f"[{label}] no runs recorded")
        return
    mean = statistics.mean(durations)
    median = statistics.median(durations)
    if len(durations) < 2:
        p95 = durations[0]
    else:
        p95 = statistics.quantiles(durations, n=100)[94]
    margin = confidence_margin(durations)
    print(
        f"[{label}] n={len(durations)} "
        f"mean={mean:.3f}s±{margin:.3f}s median={median:.3f}s p95={p95:.3f}s"
    )


def summarize_command_time(label: str, command_times: List[float], command_counts: List[int]) -> None:
    if not command_times:
        print(f"[{label}] command time: no runs recorded")
        return
    mean = statistics.mean(command_times)
    median = statistics.median(command_times)
    p95 = command_times[0] if len(command_times) < 2 else statistics.quantiles(command_times, n=100)[94]
    avg_cmds = statistics.mean(command_counts) if command_counts else 0
    margin = confidence_margin(command_times)
    print(
        f"[{label}] command time n={len(command_times)} "
        f"mean={mean:.3f}s±{margin:.3f}s median={median:.3f}s p95={p95:.3f}s avg_cmds={avg_cmds:.2f}"
    )


def summarize_command_time_per_command(label: str, command_times_per_cmd: List[float]) -> None:
    if not command_times_per_cmd:
        print(f"[{label}] command time per command: no runs recorded")
        return
    mean = statistics.mean(command_times_per_cmd)
    median = statistics.median(command_times_per_cmd)
    p95 = (
        command_times_per_cmd[0]
        if len(command_times_per_cmd) < 2
        else statistics.quantiles(command_times_per_cmd, n=100)[94]
    )
    margin = confidence_margin(command_times_per_cmd)
    print(
        f"[{label}] command time per command n={len(command_times_per_cmd)} "
        f"mean={mean:.3f}s±{margin:.3f}s median={median:.3f}s p95={p95:.3f}s"
    )


def summarize_usage(label: str, usages: List[Usage]) -> None:
    if not usages:
        print(f"[{label}] tokens: no runs recorded")
        return
    avg_input = statistics.mean(u.input_tokens for u in usages)
    avg_cached = statistics.mean(u.cached_input_tokens for u in usages)
    avg_output = statistics.mean(u.output_tokens for u in usages)
    print(
        f"[{label}] tokens avg input={avg_input:.1f} cached={avg_cached:.1f} output={avg_output:.1f}"
    )


def summarize_mcp_calls(label: str, mcp_counts: List[int]) -> None:
    if not mcp_counts:
        print(f"[{label}] mcp calls: no runs recorded")
        return
    avg_mcp = statistics.mean(mcp_counts)
    max_mcp = max(mcp_counts)
    print(f"[{label}] mcp calls avg={avg_mcp:.2f} max={max_mcp}")


def format_prompt_label(prompt: str, idx: int) -> str:
    snippet = prompt.strip()
    if len(snippet) > 60:
        snippet = snippet[:57] + "..."
    return f"prompt {idx + 1}: {snippet}"


def confidence_margin(values: List[float]) -> float:
    if len(values) < 2:
        return 0.0
    stdev = statistics.stdev(values)
    return 1.96 * stdev / math.sqrt(len(values))


def stats(values: List[float]) -> dict:
    if not values:
        return {
            "n": 0,
            "mean": None,
            "median": None,
            "p95": None,
            "margin": None,
        }
    mean = statistics.mean(values)
    median = statistics.median(values)
    if len(values) < 2:
        p95 = values[0]
    else:
        p95 = statistics.quantiles(values, n=100)[94]
    margin = confidence_margin(values)
    return {"n": len(values), "mean": mean, "median": median, "p95": p95, "margin": margin}


def safe_avg(values: List[float]) -> Optional[float]:
    return statistics.mean(values) if values else None


def fmt_stat(s: dict) -> str:
    if not s["n"]:
        return "-"
    return f"{s['mean']:.3f}±{s['margin']:.3f}"


def fmt_avg(value: Optional[float]) -> str:
    return f"{value:.2f}" if value is not None else "-"


def fmt_int(value: Optional[int]) -> str:
    return str(value) if value is not None else "-"


def render_table(headers: List[str], rows: List[List[str]]) -> None:
    widths = [len(h) for h in headers]
    for row in rows:
        for i, cell in enumerate(row):
            widths[i] = max(widths[i], len(cell))
    fmt = "  ".join(f"{{:<{w}}}" for w in widths)
    print(fmt.format(*headers))
    print(fmt.format(*["-" * w for w in widths]))
    for row in rows:
        print(fmt.format(*row))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Benchmark codex exec with/without non_login_shell_heuristic."
    )
    parser.add_argument(
        "--codex-bin",
        default="codex",
        help="Path to the codex binary (default: codex on PATH).",
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=5,
        help="Number of runs per configuration (default: 5)",
    )
    parser.add_argument(
        "--prompt",
        dest="prompts",
        action="append",
        required=True,
        help="Prompt to send to codex exec (repeat for multiple prompts).",
    )
    parser.add_argument(
        "--workdir",
        default=None,
        help="Working directory for codex exec (--cd); defaults to current dir.",
    )
    parser.add_argument(
        "--model",
        default=None,
        help="Optional model override passed to codex exec (--model).",
    )
    parser.add_argument(
        "--reasoning-effort",
        choices=["none", "minimal", "low", "medium", "high", "xhigh"],
        default=None,
        help="Optional reasoning effort override passed via -c model_reasoning_effort=...",
    )
    parser.add_argument(
        "--extra-config",
        action="append",
        default=[],
        help="Additional -c overrides passed to codex exec (repeatable).",
    )
    parser.add_argument(
        "--concurrency",
        type=int,
        default=1,
        help="Number of concurrent runs to launch per configuration (default: 1).",
    )
    parser.add_argument(
        "--skip-feature-toggle",
        action="store_true",
        help=(
            "Do not inject the non_login_shell_heuristic flag; "
            "use when benchmarking an older codex binary."
        ),
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Print codex exec stdout/stderr for debugging.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    base_cmd: List[str] = [args.codex_bin, "exec"]
    if args.workdir:
        base_cmd.extend(["--cd", args.workdir])
    if args.model:
        base_cmd.extend(["--model", args.model])
    if args.reasoning_effort:
        base_cmd.extend(["-c", f"model_reasoning_effort={args.reasoning_effort}"])
    for override in args.extra_config:
        base_cmd.extend(["-c", override])
    base_cmd.append("--json")
    toggle_feature = not args.skip_feature_toggle
    prompts = args.prompts

    print(
        f"Running {args.iterations} iterations per config for {len(prompts)} prompt(s)..."
    )
    all_off_durations: List[float] = []
    all_on_durations: List[float] = []
    all_off_command_times: List[float] = []
    all_on_command_times: List[float] = []
    all_off_command_times_per_cmd: List[float] = []
    all_on_command_times_per_cmd: List[float] = []
    all_off_command_counts: List[int] = []
    all_on_command_counts: List[int] = []
    all_off_mcp_calls: List[int] = []
    all_on_mcp_calls: List[int] = []
    all_off_usages: List[Usage] = []
    all_on_usages: List[Usage] = []
    total_off_failures = 0
    total_on_failures = 0
    per_prompt_results: list[dict] = []

    for idx, prompt in enumerate(prompts):
        label = format_prompt_label(prompt, idx)
        print(f"\nPrompt {idx + 1}/{len(prompts)}: {label}")
        (
            off_durations,
            off_cmd_times,
            off_cmd_times_per_cmd,
            off_cmd_counts,
            off_mcp_calls,
            off_usages,
            off_failures,
        ) = asyncio.run(
            run_case(
                f"login-shell | {label}",
                base_cmd,
                prompt,
                args.iterations,
                feature_enabled=False,
                toggle_feature=toggle_feature,
                concurrency=args.concurrency,
            )
        )
        (
            on_durations,
            on_cmd_times,
            on_cmd_times_per_cmd,
            on_cmd_counts,
            on_mcp_calls,
            on_usages,
            on_failures,
        ) = asyncio.run(
            run_case(
                f"non-login-shell | {label}",
                base_cmd,
                prompt,
                args.iterations,
                feature_enabled=True,
                toggle_feature=toggle_feature,
                concurrency=args.concurrency,
            )
        )
        per_prompt_results.append(
            {
                "label": label,
                "off_durations": off_durations,
                "on_durations": on_durations,
                "off_cmd_times": off_cmd_times,
                "on_cmd_times": on_cmd_times,
                "off_cmd_times_per_cmd": off_cmd_times_per_cmd,
                "on_cmd_times_per_cmd": on_cmd_times_per_cmd,
                "off_cmd_counts": off_cmd_counts,
                "on_cmd_counts": on_cmd_counts,
                "off_mcp_calls": off_mcp_calls,
                "on_mcp_calls": on_mcp_calls,
                "off_usages": off_usages,
                "on_usages": on_usages,
                "off_failures": off_failures,
                "on_failures": on_failures,
            }
        )
        all_off_durations.extend(off_durations)
        all_on_durations.extend(on_durations)
        all_off_command_times.extend(off_cmd_times)
        all_on_command_times.extend(on_cmd_times)
        all_off_command_times_per_cmd.extend(off_cmd_times_per_cmd)
        all_on_command_times_per_cmd.extend(on_cmd_times_per_cmd)
        all_off_command_counts.extend(off_cmd_counts)
        all_on_command_counts.extend(on_cmd_counts)
        all_off_mcp_calls.extend(off_mcp_calls)
        all_on_mcp_calls.extend(on_mcp_calls)
        all_off_usages.extend(off_usages)
        all_on_usages.extend(on_usages)
        total_off_failures += off_failures
        total_on_failures += on_failures

    print("\nPer-prompt summary (means ±95% CI):")
    per_prompt_rows: List[List[str]] = []
    for s in per_prompt_results:
        per_prompt_rows.append(
            [
                s["label"],
                fmt_stat(stats(s["off_durations"])),
                fmt_stat(stats(s["on_durations"])),
                fmt_stat(stats(s["off_cmd_times"])),
                fmt_stat(stats(s["on_cmd_times"])),
                fmt_stat(stats(s["off_cmd_times_per_cmd"])),
                fmt_stat(stats(s["on_cmd_times_per_cmd"])),
                fmt_avg(safe_avg(s["off_cmd_counts"])),
                fmt_avg(safe_avg(s["on_cmd_counts"])),
                fmt_avg(safe_avg(s["off_mcp_calls"])),
                fmt_avg(safe_avg(s["on_mcp_calls"])),
            ]
        )

    render_table(
        [
            "prompt",
            "wall login",
            "wall non",
            "cmd login",
            "cmd non",
            "cmd/call login",
            "cmd/call non",
            "avg cmds login",
            "avg cmds non",
            "avg mcp login",
            "avg mcp non",
        ],
        per_prompt_rows,
    )

    print("\nCombined summary (means ±95% CI):")
    combined_rows = [
        [
            "all prompts",
            fmt_stat(stats(all_off_durations)),
            fmt_stat(stats(all_on_durations)),
            fmt_stat(stats(all_off_command_times)),
            fmt_stat(stats(all_on_command_times)),
            fmt_stat(stats(all_off_command_times_per_cmd)),
            fmt_stat(stats(all_on_command_times_per_cmd)),
            fmt_avg(safe_avg(all_off_command_counts)),
            fmt_avg(safe_avg(all_on_command_counts)),
            fmt_avg(safe_avg(all_off_mcp_calls)),
            fmt_avg(safe_avg(all_on_mcp_calls)),
        ]
    ]
    render_table(
        [
            "scope",
            "wall login",
            "wall non",
            "cmd login",
            "cmd non",
            "cmd/call login",
            "cmd/call non",
            "avg cmds login",
            "avg cmds non",
            "avg mcp login",
            "avg mcp non",
        ],
        combined_rows,
    )

    print(f"\nlogin-shell failures (all prompts): {total_off_failures}")
    print(f"non-login-shell failures (all prompts): {total_on_failures}")

    return 0 if (total_off_failures + total_on_failures) == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
