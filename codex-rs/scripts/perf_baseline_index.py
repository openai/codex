#!/usr/bin/env python3
"""Build statistically robust baseline indices from perf summary.json files."""

from __future__ import annotations

import argparse
import json
import math
import re
import statistics
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class MetricSpec:
    key: str
    path: tuple[str, ...]
    higher_is_better: bool = False


METRICS: tuple[MetricSpec, ...] = (
    MetricSpec("failed_runs_total", ("failed_runs_total",), higher_is_better=False),
    MetricSpec("latency_p95_ms", ("latency_ms", "p95"), higher_is_better=False),
    MetricSpec("latency_mean_ms", ("latency_ms", "mean"), higher_is_better=False),
    MetricSpec("throughput_mean", ("throughput_runs_per_sec", "mean"), higher_is_better=True),
    MetricSpec("rss_mean_mb", ("max_rss_mb", "mean"), higher_is_better=False),
    MetricSpec("peak_fds_mean", ("peak_open_fds", "mean"), higher_is_better=False),
    MetricSpec("peak_children_mean", ("peak_direct_children", "mean"), higher_is_better=False),
    MetricSpec("sampled_peak_tree_rss_mean_mb", ("process_tree_sampled", "peak_tree_rss_mb", "mean"), higher_is_better=False),
    MetricSpec("sampled_peak_tree_cpu_p95", ("process_tree_sampled", "peak_tree_cpu_pct", "p95"), higher_is_better=False),
    MetricSpec("top_peak_cpu_p95", ("top_attach", "peak_cpu_pct", "p95"), higher_is_better=False),
    MetricSpec("monitor_loop_mean_ms", ("worker_step_timings_ms", "monitor_loop", "mean"), higher_is_better=False),
)


def _num(value: Any) -> float | None:
    if isinstance(value, (int, float)):
        v = float(value)
        if math.isfinite(v):
            return v
    return None


def _extract(d: dict[str, Any], path: tuple[str, ...]) -> float | None:
    cur: Any = d
    for p in path:
        if not isinstance(cur, dict):
            return None
        cur = cur.get(p)
    return _num(cur)


def _percentile(values: list[float], p: float) -> float:
    if not values:
        return math.nan
    if len(values) == 1:
        return values[0]
    s = sorted(values)
    idx = (len(s) - 1) * p
    lo = int(math.floor(idx))
    hi = int(math.ceil(idx))
    if lo == hi:
        return s[lo]
    frac = idx - lo
    return s[lo] * (1 - frac) + s[hi] * frac


def _trimmed_mean(values: list[float], trim_ratio: float = 0.10) -> float:
    if not values:
        return math.nan
    s = sorted(values)
    k = int(len(s) * trim_ratio)
    core = s[k : len(s) - k] if len(s) - 2 * k > 0 else s
    return statistics.fmean(core)


def _mad(values: list[float], median: float) -> float:
    if not values:
        return math.nan
    abs_dev = [abs(v - median) for v in values]
    return _percentile(abs_dev, 0.50)


def _iqr(values: list[float]) -> float:
    if not values:
        return math.nan
    return _percentile(values, 0.75) - _percentile(values, 0.25)


def _status(current: float, median: float, robust_z: float | None, higher_is_better: bool) -> str:
    if robust_z is None:
        return "insufficient_data"
    bad = (current < median and higher_is_better) or (current > median and not higher_is_better)
    z = abs(robust_z)
    if bad and z >= 3.0:
        return "red"
    if bad and z >= 1.5:
        return "yellow"
    return "green"


def _sort_key(payload: dict[str, Any]) -> str:
    return str(payload.get("generated_at") or "")


def _normalize_profile_name(name: str) -> str:
    # Remove common timestamp suffixes like -20260223-031755.
    return re.sub(r"-\\d{8}-\\d{6}$", "", name.strip())


def _bucket_profile_name(name: str, mode: str) -> str:
    normalized = _normalize_profile_name(name).lower()
    if mode == "exact":
        return _normalize_profile_name(name)
    if "stream" in normalized:
        return "streaming"
    if "throughput" in normalized:
        return "throughput"
    if "cold" in normalized or "startup" in normalized:
        return "cold"
    if "exec" in normalized:
        return "exec"
    if "smoke" in normalized:
        return "smoke"
    if "stress" in normalized:
        return "stress"
    return _normalize_profile_name(name)


def main() -> int:
    ap = argparse.ArgumentParser(description="Build robust perf baseline indices from summary.json files")
    ap.add_argument("--summary", action="append", default=[], help="Path to summary.json (repeatable)")
    ap.add_argument(
        "--summary-glob",
        action="append",
        default=[],
        help="Glob for summary.json files (repeatable), e.g. 'codex-rs/perf-results/*/summary.json'",
    )
    ap.add_argument("--window", type=int, default=30, help="Rolling window size per profile")
    ap.add_argument("--min-samples", type=int, default=5, help="Minimum samples to compute robust z-score")
    ap.add_argument(
        "--group-mode",
        choices=("heuristic", "exact"),
        default="heuristic",
        help="Profile grouping mode for baseline aggregation.",
    )
    ap.add_argument("--out", required=True, help="Output JSON path")
    args = ap.parse_args()

    summary_paths: set[Path] = set()
    for raw in args.summary:
        summary_paths.add(Path(raw))
    for pattern in args.summary_glob:
        for matched in Path().glob(pattern):
            if matched.is_file() and matched.name == "summary.json":
                summary_paths.add(matched)
    if not summary_paths:
        raise SystemExit("No summary files resolved from --summary/--summary-glob")

    grouped: dict[str, list[dict[str, Any]]] = {}
    for p in sorted(summary_paths):
        payload = json.loads(p.read_text())
        profile = payload.get("profile", {}) if isinstance(payload, dict) else {}
        raw_name = str(profile.get("name") or p.parent.name)
        name = _bucket_profile_name(raw_name, args.group_mode)
        payload["_path"] = str(p)
        payload["_raw_profile_name"] = raw_name
        grouped.setdefault(name, []).append(payload)

    out: dict[str, Any] = {
        "generated_at": __import__("datetime").datetime.now(__import__("datetime").timezone.utc).isoformat(),
        "window": args.window,
        "min_samples": args.min_samples,
        "profiles": {},
    }

    for profile_name, rows in grouped.items():
        rows = sorted(rows, key=_sort_key)[-args.window :]
        latest = rows[-1]
        summary_rows = [r.get("summary", {}) for r in rows if isinstance(r.get("summary"), dict)]

        profile_out: dict[str, Any] = {
            "sample_count": len(summary_rows),
            "latest_summary_path": latest.get("_path"),
            "metrics": {},
        }

        for spec in METRICS:
            values = [v for s in summary_rows if (v := _extract(s, spec.path)) is not None]
            if not values:
                profile_out["metrics"][spec.key] = {"present": False}
                continue

            current = values[-1]
            median = _percentile(values, 0.50)
            mad = _mad(values, median)
            robust_sigma = mad * 1.4826 if math.isfinite(mad) and mad > 0 else None
            robust_z = (current - median) / robust_sigma if robust_sigma else None
            mean = statistics.fmean(values)
            std = statistics.pstdev(values) if len(values) > 1 else 0.0
            cv = (std / abs(mean)) if mean != 0 else None
            delta_pct = ((current - median) / abs(median) * 100.0) if median not in (0.0, -0.0) else None

            metric_out = {
                "present": True,
                "higher_is_better": spec.higher_is_better,
                "n": len(values),
                "current": current,
                "mean": mean,
                "median": median,
                "trimmed_mean_10": _trimmed_mean(values, 0.10),
                "p95": _percentile(values, 0.95),
                "p99": _percentile(values, 0.99),
                "min": min(values),
                "max": max(values),
                "std": std,
                "cv": cv,
                "mad": mad,
                "iqr": _iqr(values),
                "delta_pct_from_median": delta_pct,
                "robust_z": robust_z,
                "status": _status(current, median, robust_z, spec.higher_is_better)
                if len(values) >= args.min_samples
                else "insufficient_data",
            }
            profile_out["metrics"][spec.key] = metric_out

        out["profiles"][profile_name] = profile_out

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8")
    print(f"baseline_index_json={out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
