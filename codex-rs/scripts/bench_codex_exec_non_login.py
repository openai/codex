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
import statistics
import sys
import time
from dataclasses import dataclass
from typing import List


@dataclass
class RunResult:
    duration: float
    exit_code: int
    stderr: str


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
        _, stderr = await proc.communicate()
        duration = time.perf_counter() - start
        stderr_text = stderr.decode(errors="replace").strip()
        return RunResult(duration=duration, exit_code=proc.returncode, stderr=stderr_text)


async def run_case(
    label: str,
    base_cmd: List[str],
    prompt: str,
    iterations: int,
    feature_enabled: bool,
    toggle_feature: bool,
    concurrency: int,
) -> tuple[list[float], int]:
    sem = asyncio.Semaphore(concurrency)
    tasks = [
        asyncio.create_task(
            run_once(base_cmd, prompt, feature_enabled, toggle_feature, sem)
        )
        for _ in range(iterations)
    ]
    durations: List[float] = []
    failures = 0
    for idx, task in enumerate(asyncio.as_completed(tasks), start=1):
        result = await task
        durations.append(result.duration)
        status = "ok" if result.exit_code == 0 else f"fail ({result.exit_code})"
        print(f"[{label}] run {idx}/{iterations}: {result.duration:.3f}s [{status}]")
        if result.exit_code != 0:
            failures += 1
            if result.stderr:
                print(f"    stderr: {result.stderr}", file=sys.stderr)
    return durations, failures


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
    print(
        f"[{label}] n={len(durations)} "
        f"mean={mean:.3f}s median={median:.3f}s p95={p95:.3f}s"
    )


def format_prompt_label(prompt: str, idx: int) -> str:
    snippet = prompt.strip()
    if len(snippet) > 60:
        snippet = snippet[:57] + "..."
    return f"prompt {idx + 1}: {snippet}"


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
    if not args.verbose:
        base_cmd.append("--json")
    toggle_feature = not args.skip_feature_toggle
    prompts = args.prompts

    print(
        f"Running {args.iterations} iterations per config for {len(prompts)} prompt(s)..."
    )
    all_off_durations: List[float] = []
    all_on_durations: List[float] = []
    total_off_failures = 0
    total_on_failures = 0
    per_prompt_results: list[dict] = []

    for idx, prompt in enumerate(prompts):
        label = format_prompt_label(prompt, idx)
        print(f"\nPrompt {idx + 1}/{len(prompts)}: {label}")
        off_durations, off_failures = asyncio.run(
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
        on_durations, on_failures = asyncio.run(
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
                "off_failures": off_failures,
                "on_failures": on_failures,
            }
        )
        all_off_durations.extend(off_durations)
        all_on_durations.extend(on_durations)
        total_off_failures += off_failures
        total_on_failures += on_failures

    print("\nPer-prompt summary:")
    for stats in per_prompt_results:
        summarize(f"login-shell | {stats['label']}", stats["off_durations"])
        summarize(f"non-login-shell | {stats['label']}", stats["on_durations"])
        print(f"login-shell failures ({stats['label']}): {stats['off_failures']}")
        print(f"non-login-shell failures ({stats['label']}): {stats['on_failures']}")

    print("\nCombined summary across prompts:")
    summarize("login-shell (all prompts)", all_off_durations)
    summarize("non-login-shell (all prompts)", all_on_durations)
    print(f"login-shell failures (all prompts): {total_off_failures}")
    print(f"non-login-shell failures (all prompts): {total_on_failures}")

    return 0 if (total_off_failures + total_on_failures) == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
