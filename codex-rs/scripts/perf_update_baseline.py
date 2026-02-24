#!/usr/bin/env python3
"""Append/update a run row in BASELINE_COMPARISON.md from summary.json."""

from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


TABLE_HEADER = (
    "| Date | Run folder | Workload profile | Latency p50 (ms) | Latency p95 (ms) | "
    "Throughput mean (runs/s) | Max RSS p50 (KB) | Queue/cancel datapoints | Delta summary |"
)
TABLE_SEPARATOR = "|---|---|---|---:|---:|---:|---:|---:|---|"


def read_summary(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def as_number(value: Any) -> str:
    if isinstance(value, (int, float)):
        return f"{value:.3f}" if isinstance(value, float) else str(value)
    return "n/a"


def infer_workload(summary: dict[str, Any]) -> str:
    profile = summary.get("profile", {})
    if isinstance(profile, dict):
        name = profile.get("name")
        phase = profile.get("phase")
        concurrency = profile.get("concurrency")
        if any(item is not None and str(item).strip() for item in (name, phase, concurrency)):
            profile_name = str(name).strip() if name is not None else ""
            profile_phase = str(phase).strip() if phase is not None else ""
            if isinstance(concurrency, (int, float)):
                concurrency_text = str(int(concurrency))
            elif concurrency is None:
                concurrency_text = ""
            else:
                concurrency_text = str(concurrency).strip()

            parts = [part for part in (profile_name, profile_phase) if part]
            if concurrency_text:
                parts.append(f"c={concurrency_text}")
            if parts:
                return "profile:" + "/".join(parts)

    command = str(summary.get("command", "")).strip()
    return command if command else "unknown"


def queue_cancel_count(summary: dict[str, Any]) -> int:
    metrics = summary.get("summary", {}).get("queue_cancel_metrics", {})
    if not isinstance(metrics, dict):
        return 0
    return sum(int(v) for v in metrics.values() if isinstance(v, (int, float)))


def run_folder_name(summary_path: Path) -> str:
    return summary_path.parent.name


def build_row(summary: dict[str, Any], summary_path: Path, workload: str, delta: str) -> str:
    stats = summary.get("summary", {})
    latency = stats.get("latency_ms", {})
    throughput = stats.get("throughput_runs_per_sec", {})
    rss = stats.get("max_rss_kb", {})

    date = datetime.now(timezone.utc).date().isoformat()
    run_folder = f"`{run_folder_name(summary_path)}`"
    workload_cell = workload.replace("|", "\\|")
    return (
        f"| {date} | {run_folder} | {workload_cell} | "
        f"{as_number(latency.get('p50'))} | {as_number(latency.get('p95'))} | "
        f"{as_number(throughput.get('mean'))} | {as_number(rss.get('p50'))} | "
        f"{queue_cancel_count(summary)} | {delta} |"
    )


def upsert_row(report_text: str, row: str, run_folder: str) -> str:
    lines = report_text.splitlines()
    header_idx = None
    separator_idx = None

    for idx, line in enumerate(lines):
        if line.strip() == TABLE_HEADER:
            header_idx = idx
            break

    if header_idx is None:
        if lines and lines[-1].strip():
            lines.append("")
        lines.extend(
            [
                "## Delta Tracking Log",
                "",
                TABLE_HEADER,
                TABLE_SEPARATOR,
                row,
            ]
        )
        return "\n".join(lines) + "\n"

    if header_idx + 1 < len(lines) and lines[header_idx + 1].strip() == TABLE_SEPARATOR:
        separator_idx = header_idx + 1
    else:
        lines.insert(header_idx + 1, TABLE_SEPARATOR)
        separator_idx = header_idx + 1

    table_start = separator_idx + 1
    table_end = table_start
    while table_end < len(lines) and lines[table_end].startswith("|"):
        table_end += 1

    run_token = f"`{run_folder}`"
    for idx in range(table_start, table_end):
        if run_token in lines[idx]:
            lines[idx] = row
            return "\n".join(lines) + "\n"

    lines.insert(table_end, row)
    return "\n".join(lines) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Append/update BASELINE_COMPARISON.md row from a summary.json run artifact."
    )
    parser.add_argument("--summary-json", required=True, help="Path to summary.json")
    parser.add_argument(
        "--report",
        default="codex-rs/perf-results/BASELINE_COMPARISON.md",
        help="Path to comparison markdown report",
    )
    parser.add_argument("--workload", default=None, help="Override workload profile text")
    parser.add_argument("--delta-summary", default="Update", help="Delta summary text")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    summary_path = Path(args.summary_json)
    report_path = Path(args.report)

    if not summary_path.exists():
        raise FileNotFoundError(f"summary.json not found: {summary_path}")

    summary = read_summary(summary_path)
    workload = args.workload or infer_workload(summary)
    row = build_row(summary, summary_path, workload=workload, delta=args.delta_summary)

    existing = report_path.read_text(encoding="utf-8") if report_path.exists() else ""
    updated = upsert_row(existing, row, run_folder_name(summary_path))
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(updated, encoding="utf-8")
    print(f"updated_report={report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
