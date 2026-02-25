#!/usr/bin/env python3
"""Run named SLA profiles against a perf summary."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path


PROFILE_ARGS_BASE: dict[str, list[str]] = {
    "cold": [
        "--max-failed-runs",
        "0",
        "--max-latency-p95-ms",
        "1000",
        "--min-throughput-mean",
        "2.0",
        "--max-peak-fds-mean",
        "6",
    ],
    "throughput": [
        "--max-failed-runs",
        "0",
        "--max-latency-p95-ms",
        "1200",
        "--min-throughput-mean",
        "8.0",
        "--max-peak-fds-mean",
        "6",
    ],
    "exec": [
        "--max-failed-runs",
        "0",
        "--max-latency-p95-ms",
        "8000",
        "--min-throughput-mean",
        "0.14",
        "--max-peak-children-mean",
        "1.5",
    ],
    "streaming": [
        "--max-failed-runs",
        "0",
        "--max-latency-p95-ms",
        "25000",
        "--min-throughput-mean",
        "0.03",
        "--min-turn-metric-points-mean",
        "1",
    ],
}


PROFILE_ARGS_GRANULAR: dict[str, list[str]] = {
    "cold": [
        "--max-build-cmd-mean-ms",
        "25",
        "--max-spawn-proc-mean-ms",
        "250",
        "--max-monitor-loop-mean-ms",
        "2500",
        "--max-communicate-mean-ms",
        "600",
        "--max-parse-stats-mean-ms",
        "300",
    ],
    "throughput": [
        "--max-build-cmd-mean-ms",
        "25",
        "--max-spawn-proc-mean-ms",
        "250",
        "--max-monitor-loop-mean-ms",
        "3000",
        "--max-communicate-mean-ms",
        "700",
        "--max-parse-stats-mean-ms",
        "300",
    ],
    "exec": [
        "--min-turn-metric-points-mean",
        "1",
        "--max-build-cmd-mean-ms",
        "30",
        "--max-spawn-proc-mean-ms",
        "300",
        "--max-monitor-loop-mean-ms",
        "7000",
        "--max-communicate-mean-ms",
        "1000",
        "--max-parse-stats-mean-ms",
        "400",
    ],
    "streaming": [
        "--max-monitor-loop-mean-ms",
        "30000",
        "--max-parse-stats-mean-ms",
        "500",
        "--max-sampled-peak-tree-rss-mean-mb",
        "8",
    ],
}

PROFILE_ARGS_TOP_ATTACH: dict[str, list[str]] = {
    "cold": [
        "--min-top-sample-count-mean",
        "1",
        "--max-top-peak-cpu-p95",
        "120",
    ],
    "throughput": [
        "--min-top-sample-count-mean",
        "1",
        "--max-top-peak-cpu-p95",
        "220",
    ],
    "exec": [
        "--min-top-sample-count-mean",
        "1",
        "--max-top-peak-cpu-p95",
        "260",
    ],
    "streaming": [
        "--min-top-sample-count-mean",
        "1",
        "--max-top-peak-cpu-p95",
        "260",
    ],
}

PROFILE_ARGS_VMmap_XCTRACE: dict[str, list[str]] = {
    "cold": [
        "--min-xctrace-trace-count",
        "1",
    ],
    "throughput": [],
    "exec": [
        "--min-xctrace-trace-count",
        "1",
    ],
    "streaming": [],
}


def main() -> int:
    parser = argparse.ArgumentParser(description="Run named perf SLA profile checks.")
    parser.add_argument("--summary", help="Path to summary.json (single-profile mode)")
    parser.add_argument("--cold-summary", help="Path to cold profile summary.json (all-map mode)")
    parser.add_argument(
        "--throughput-summary",
        help="Path to throughput profile summary.json (all-map mode)",
    )
    parser.add_argument("--exec-summary", help="Path to exec profile summary.json (all-map mode)")
    parser.add_argument(
        "--streaming-summary",
        help="Path to streaming profile summary.json (all-map mode)",
    )
    parser.add_argument(
        "--profile",
        required=True,
        choices=sorted([*PROFILE_ARGS_BASE.keys(), "all"]),
        help="SLA profile name",
    )
    parser.add_argument(
        "--extra",
        nargs="*",
        default=[],
        help="Extra raw args forwarded to perf_sla_check.py",
    )
    args = parser.parse_args()

    check_script = Path(__file__).with_name("perf_sla_check.py")

    def run_profile(profile: str, summary: Path) -> int:
        payload = json.loads(summary.read_text())
        summary_payload = payload.get("summary", {})
        has_granular = bool(summary_payload.get("worker_step_timings_ms"))
        has_top_attach = bool(summary_payload.get("top_attach"))
        has_vmmap = bool(summary_payload.get("vmmap_snapshots"))
        has_xctrace = bool(summary_payload.get("xctrace", {}).get("trace_count"))
        profile_args = list(PROFILE_ARGS_BASE[profile])
        if has_granular:
            profile_args.extend(PROFILE_ARGS_GRANULAR[profile])
        if has_top_attach:
            profile_args.extend(PROFILE_ARGS_TOP_ATTACH[profile])
        if has_vmmap and has_xctrace:
            profile_args.extend(PROFILE_ARGS_VMmap_XCTRACE[profile])
        cmd = [
            sys.executable,
            str(check_script),
            "--summary",
            str(summary),
            "--label",
            profile,
            *profile_args,
            *args.extra,
        ]
        print("running:", " ".join(cmd))
        proc = subprocess.run(cmd, check=False)
        return proc.returncode

    if args.profile != "all":
        if not args.summary:
            parser.error("--summary is required when --profile is not 'all'")
        return run_profile(args.profile, Path(args.summary))

    summary_map = {
        "cold": args.cold_summary,
        "throughput": args.throughput_summary,
        "exec": args.exec_summary,
        "streaming": args.streaming_summary,
    }
    missing = [name for name, path in summary_map.items() if not path]
    if missing:
        parser.error(
            "--profile all requires --cold-summary --throughput-summary --exec-summary --streaming-summary"
        )

    results: dict[str, int] = {}
    for name in ("cold", "throughput", "exec", "streaming"):
        results[name] = run_profile(name, Path(summary_map[name]))

    print("")
    print("profile matrix:")
    overall_ok = True
    for name in ("cold", "throughput", "exec", "streaming"):
        ok = results[name] == 0
        overall_ok = overall_ok and ok
        print(f"- {name}: {'PASS' if ok else 'FAIL'}")
    return 0 if overall_ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
