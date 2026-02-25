#!/usr/bin/env python3
"""Generate warning/suggestion report from perf baseline index JSON."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def _metric(metrics: dict[str, Any], key: str) -> dict[str, Any] | None:
    item = metrics.get(key)
    if isinstance(item, dict) and item.get("present"):
        return item
    return None


def _is_warn(item: dict[str, Any] | None) -> bool:
    if not item:
        return False
    return item.get("status") in {"yellow", "red"}


def _severity(item: dict[str, Any] | None) -> str:
    if not item:
        return "info"
    return str(item.get("status") or "info")


def main() -> int:
    ap = argparse.ArgumentParser(description="Generate bottleneck warnings/suggestions from baseline index")
    ap.add_argument("--index", required=True, help="Path to perf baseline index JSON")
    ap.add_argument("--out", required=True, help="Output JSON path")
    args = ap.parse_args()

    payload = json.loads(Path(args.index).read_text())
    profiles = payload.get("profiles", {}) if isinstance(payload, dict) else {}

    out: dict[str, Any] = {
        "generated_from": str(args.index),
        "warnings": [],
    }

    for profile_name, profile in profiles.items():
        metrics = profile.get("metrics", {}) if isinstance(profile, dict) else {}

        latency = _metric(metrics, "latency_p95_ms")
        throughput = _metric(metrics, "throughput_mean")
        monitor = _metric(metrics, "monitor_loop_mean_ms")
        rss = _metric(metrics, "rss_mean_mb")
        fds = _metric(metrics, "peak_fds_mean")
        tree_rss = _metric(metrics, "sampled_peak_tree_rss_mean_mb")
        tree_cpu = _metric(metrics, "sampled_peak_tree_cpu_p95")
        failures = _metric(metrics, "failed_runs_total")
        spawn = _metric(metrics, "spawn_proc_mean_ms")
        parse = _metric(metrics, "parse_stats_mean_ms")

        if failures and (failures.get("current") or 0) > 0:
            out["warnings"].append(
                {
                    "profile": profile_name,
                    "severity": "red",
                    "signal": "failed_runs_total",
                    "message": "Non-zero failures detected.",
                    "suggestion": "Inspect stderr tails first, then split auth/network vs runtime failures.",
                }
            )

        if _is_warn(latency) and _is_warn(throughput):
            out["warnings"].append(
                {
                    "profile": profile_name,
                    "severity": "red" if _severity(latency) == "red" else "yellow",
                    "signal": "latency_p95_ms+throughput_mean",
                    "message": "Latency regressed while throughput dropped.",
                    "suggestion": "Check lock contention, monitor-loop overhead, and child process churn; compare worker step timing deltas.",
                }
            )

        if _is_warn(monitor):
            out["warnings"].append(
                {
                    "profile": profile_name,
                    "severity": _severity(monitor),
                    "signal": "monitor_loop_mean_ms",
                    "message": "Monitor loop overhead increased.",
                    "suggestion": "Increase probe interval/sleep, reduce subprocess probes, or move to event-driven sampling.",
                }
            )

        if _is_warn(spawn):
            out["warnings"].append(
                {
                    "profile": profile_name,
                    "severity": _severity(spawn),
                    "signal": "spawn_proc_mean_ms",
                    "message": "Process spawn time drifted upward.",
                    "suggestion": "Use pooled workers or reduce per-iteration process start/teardown.",
                }
            )

        if _is_warn(parse):
            out["warnings"].append(
                {
                    "profile": profile_name,
                    "severity": _severity(parse),
                    "signal": "parse_stats_mean_ms",
                    "message": "Stats parsing cost drifted upward.",
                    "suggestion": "Simplify parsing regexes and defer non-critical parsing.",
                }
            )

        if _is_warn(rss) or _is_warn(tree_rss):
            out["warnings"].append(
                {
                    "profile": profile_name,
                    "severity": "red" if _severity(rss) == "red" or _severity(tree_rss) == "red" else "yellow",
                    "signal": "rss_mean_mb/sampled_peak_tree_rss_mean_mb",
                    "message": "Memory footprint increased versus baseline.",
                    "suggestion": "Inspect vmmap progression and allocation hotspots; check retained buffers and long-lived maps.",
                }
            )

        if _is_warn(tree_cpu):
            out["warnings"].append(
                {
                    "profile": profile_name,
                    "severity": _severity(tree_cpu),
                    "signal": "sampled_peak_tree_cpu_p95",
                    "message": "CPU tail increased in process tree.",
                    "suggestion": "Use xctrace hotspots to isolate frames; prioritize hot loops before lock tuning.",
                }
            )

        if _is_warn(fds):
            out["warnings"].append(
                {
                    "profile": profile_name,
                    "severity": _severity(fds),
                    "signal": "peak_fds_mean",
                    "message": "File descriptor usage drifted upward.",
                    "suggestion": "Audit descriptors in worker subprocesses and verify close-on-exec behavior.",
                }
            )

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8")
    print(f"bottleneck_suggestions_json={out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
