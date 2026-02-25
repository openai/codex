#!/usr/bin/env python3
"""Validate perf summary JSON against explicit SLA thresholds."""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path


def _num(value):
    if value is None:
        return None
    if isinstance(value, (int, float)):
        if isinstance(value, float) and math.isnan(value):
            return None
        return float(value)
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description="Check perf summary against SLA thresholds.")
    parser.add_argument("--summary", required=True, help="Path to benchmark summary.json")
    parser.add_argument("--label", default="perf", help="Human label for reporting")
    parser.add_argument("--max-failed-runs", type=int, default=0)
    parser.add_argument("--max-latency-p95-ms", type=float)
    parser.add_argument("--max-latency-mean-ms", type=float)
    parser.add_argument("--min-throughput-mean", type=float)
    parser.add_argument("--max-rss-mean-mb", type=float)
    parser.add_argument("--max-peak-fds-mean", type=float)
    parser.add_argument("--max-peak-children-mean", type=float)
    parser.add_argument("--min-turn-metric-points-mean", type=float)
    parser.add_argument("--min-action-metric-points-mean", type=float)
    parser.add_argument("--min-stream-metric-points-mean", type=float)
    parser.add_argument("--max-turn-metric-value-sum-mean", type=float)
    parser.add_argument("--max-action-metric-value-sum-mean", type=float)
    parser.add_argument("--max-stream-metric-value-sum-mean", type=float)
    parser.add_argument("--max-sampled-peak-tree-rss-mean-mb", type=float)
    parser.add_argument("--max-sampled-peak-tree-cpu-p95", type=float)
    parser.add_argument("--max-build-cmd-mean-ms", type=float)
    parser.add_argument("--max-spawn-proc-mean-ms", type=float)
    parser.add_argument("--max-monitor-loop-mean-ms", type=float)
    parser.add_argument("--max-communicate-mean-ms", type=float)
    parser.add_argument("--max-parse-stats-mean-ms", type=float)
    parser.add_argument("--min-top-sample-count-mean", type=float)
    parser.add_argument("--max-top-peak-rss-mean-mb", type=float)
    parser.add_argument("--max-top-peak-cpu-p95", type=float)
    parser.add_argument("--max-top-mean-cpu-mean", type=float)
    parser.add_argument("--max-vmmap-start-physical-mean-mb", type=float)
    parser.add_argument("--max-vmmap-mid-physical-mean-mb", type=float)
    parser.add_argument("--max-vmmap-end-physical-mean-mb", type=float)
    parser.add_argument("--min-xctrace-trace-count", type=float)
    args = parser.parse_args()

    path = Path(args.summary)
    payload = json.loads(path.read_text())
    summary = payload.get("summary", {})

    failed_runs = int(summary.get("failed_runs_total", 0))
    latency = summary.get("latency_ms", {})
    throughput = summary.get("throughput_runs_per_sec", {})
    rss = summary.get("max_rss_mb", summary.get("max_rss_kb", {}))
    fds = summary.get("peak_open_fds", {})
    children = summary.get("peak_direct_children", {})
    tas = summary.get("otel_turn_action_stream", {})
    turn_points = tas.get("turn_metric_points", {})
    action_points = tas.get("action_metric_points", {})
    stream_points = tas.get("stream_metric_points", {})
    turn_value_sum = tas.get("turn_metric_value_sum", {})
    action_value_sum = tas.get("action_metric_value_sum", {})
    stream_value_sum = tas.get("stream_metric_value_sum", {})
    process_tree_sampled = summary.get("process_tree_sampled", {})
    worker_step_timings = summary.get("worker_step_timings_ms", {})
    top_attach = summary.get("top_attach", {})
    vmmap_snapshots = summary.get("vmmap_snapshots", {})
    xctrace = summary.get("xctrace", {})

    checks = []

    def check_max(name: str, value, limit):
        value_num = _num(value)
        if limit is None:
            return
        if value_num is None:
            checks.append((False, f"{name} unavailable; limit={limit}"))
            return
        checks.append((value_num <= limit, f"{name}={value_num:.6g} <= {limit:.6g}"))

    def check_min(name: str, value, limit):
        value_num = _num(value)
        if limit is None:
            return
        if value_num is None:
            checks.append((False, f"{name} unavailable; limit={limit}"))
            return
        checks.append((value_num >= limit, f"{name}={value_num:.6g} >= {limit:.6g}"))

    checks.append(
        (
            failed_runs <= args.max_failed_runs,
            f"failed_runs_total={failed_runs} <= {args.max_failed_runs}",
        )
    )
    check_max("latency_p95_ms", latency.get("p95"), args.max_latency_p95_ms)
    check_max("latency_mean_ms", latency.get("mean"), args.max_latency_mean_ms)
    check_min("throughput_mean", throughput.get("mean"), args.min_throughput_mean)
    check_max("rss_mean_mb", rss.get("mean"), args.max_rss_mean_mb)
    check_max("peak_fds_mean", fds.get("mean"), args.max_peak_fds_mean)
    check_max("peak_children_mean", children.get("mean"), args.max_peak_children_mean)
    check_min("turn_metric_points_mean", turn_points.get("mean"), args.min_turn_metric_points_mean)
    check_min("action_metric_points_mean", action_points.get("mean"), args.min_action_metric_points_mean)
    check_min("stream_metric_points_mean", stream_points.get("mean"), args.min_stream_metric_points_mean)
    check_max("turn_metric_value_sum_mean", turn_value_sum.get("mean"), args.max_turn_metric_value_sum_mean)
    check_max(
        "action_metric_value_sum_mean",
        action_value_sum.get("mean"),
        args.max_action_metric_value_sum_mean,
    )
    check_max(
        "stream_metric_value_sum_mean",
        stream_value_sum.get("mean"),
        args.max_stream_metric_value_sum_mean,
    )
    check_max(
        "sampled_peak_tree_rss_mean_mb",
        process_tree_sampled.get("peak_tree_rss_mb", {}).get("mean"),
        args.max_sampled_peak_tree_rss_mean_mb,
    )
    check_max(
        "sampled_peak_tree_cpu_p95",
        process_tree_sampled.get("peak_tree_cpu_pct", {}).get("p95"),
        args.max_sampled_peak_tree_cpu_p95,
    )
    check_max("build_cmd_mean_ms", worker_step_timings.get("build_cmd", {}).get("mean"), args.max_build_cmd_mean_ms)
    check_max(
        "spawn_proc_mean_ms",
        worker_step_timings.get("spawn_proc", {}).get("mean"),
        args.max_spawn_proc_mean_ms,
    )
    check_max(
        "monitor_loop_mean_ms",
        worker_step_timings.get("monitor_loop", {}).get("mean"),
        args.max_monitor_loop_mean_ms,
    )
    check_max(
        "communicate_mean_ms",
        worker_step_timings.get("communicate", {}).get("mean"),
        args.max_communicate_mean_ms,
    )
    check_max(
        "parse_stats_mean_ms",
        worker_step_timings.get("parse_stats", {}).get("mean"),
        args.max_parse_stats_mean_ms,
    )
    check_min("top_sample_count_mean", top_attach.get("sample_count", {}).get("mean"), args.min_top_sample_count_mean)
    check_max("top_peak_rss_mean_mb", top_attach.get("peak_rss_mb", {}).get("mean"), args.max_top_peak_rss_mean_mb)
    check_max("top_peak_cpu_p95", top_attach.get("peak_cpu_pct", {}).get("p95"), args.max_top_peak_cpu_p95)
    check_max("top_mean_cpu_mean", top_attach.get("mean_cpu_pct", {}).get("mean"), args.max_top_mean_cpu_mean)
    check_max(
        "vmmap_start_physical_mean_mb",
        vmmap_snapshots.get("start_physical_footprint_mb", {}).get("mean"),
        args.max_vmmap_start_physical_mean_mb,
    )
    check_max(
        "vmmap_mid_physical_mean_mb",
        vmmap_snapshots.get("mid_physical_footprint_mb", {}).get("mean"),
        args.max_vmmap_mid_physical_mean_mb,
    )
    check_max(
        "vmmap_end_physical_mean_mb",
        vmmap_snapshots.get("end_physical_footprint_mb", {}).get("mean"),
        args.max_vmmap_end_physical_mean_mb,
    )
    check_min("xctrace_trace_count", xctrace.get("trace_count"), args.min_xctrace_trace_count)

    failed = [msg for ok, msg in checks if not ok]
    passed = [msg for ok, msg in checks if ok]

    print(f"[{args.label}] summary={path}")
    for msg in passed:
        print(f"PASS: {msg}")
    for msg in failed:
        print(f"FAIL: {msg}")

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
