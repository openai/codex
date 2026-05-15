#!/usr/bin/env python3
"""Measure latency for production plugin APIs used by Codex plugin loading."""

from __future__ import annotations

import argparse
import csv
import json
import math
import statistics
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


DEFAULT_BASE_URL = "https://chatgpt.com"
DEFAULT_AUTH_JSON = "~/.codex/auth.json"
DEFAULT_OUTPUT_DIR = "/tmp"


@dataclass(frozen=True)
class ProbeEndpoint:
    label: str
    path: str
    timeout_sec: float

    def api_label(self) -> str:
        return f"{self.label} | GET {self.path}"


ENDPOINTS: tuple[ProbeEndpoint, ...] = (
    ProbeEndpoint(
        label="workspace_installed_for_shared",
        path="/backend-api/ps/plugins/installed?scope=WORKSPACE",
        timeout_sec=30.0,
    ),
    ProbeEndpoint(
        label="workspace_shared",
        path="/backend-api/ps/plugins/workspace/shared?limit=200",
        timeout_sec=30.0,
    ),
    ProbeEndpoint(
        label="workspace_directory",
        path="/backend-api/ps/plugins/list?scope=WORKSPACE&limit=200",
        timeout_sec=30.0,
    ),
    ProbeEndpoint(
        label="global_directory",
        path="/backend-api/ps/plugins/list?scope=GLOBAL&limit=200",
        timeout_sec=30.0,
    ),
    ProbeEndpoint(
        label="global_installed",
        path="/backend-api/ps/plugins/installed?scope=GLOBAL",
        timeout_sec=30.0,
    ),
    ProbeEndpoint(
        label="featured_plugin_ids",
        path="/backend-api/plugins/featured?platform=codex",
        timeout_sec=10.0,
    ),
    ProbeEndpoint(
        label="created_by_me_workspace",
        path="/backend-api/ps/plugins/workspace/created?limit=200",
        timeout_sec=30.0,
    ),
    ProbeEndpoint(
        label="workspace_installed_after_created",
        path="/backend-api/ps/plugins/installed?scope=WORKSPACE",
        timeout_sec=30.0,
    ),
)


CSV_FIELDS = (
    "iteration",
    "order",
    "label",
    "method",
    "url_path_query",
    "started_at",
    "ended_at",
    "latency_ms",
    "latency_ns",
    "status",
    "success",
    "response_bytes",
    "request_id",
    "error",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Sequentially benchmark production Codex plugin-loading APIs. "
            "The default run performs 100 iterations of all endpoints."
        )
    )
    parser.add_argument("--iterations", type=int, default=100)
    parser.add_argument("--call-gap-sec", type=float, default=1.0)
    parser.add_argument("--iteration-gap-sec", type=float, default=10.0)
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--auth-json", default=DEFAULT_AUTH_JSON)
    parser.add_argument("--output-dir", default=DEFAULT_OUTPUT_DIR)
    parser.add_argument(
        "--user-agent",
        default="codex-plugin-api-latency-probe/1.0",
        help="User-Agent sent to production APIs.",
    )
    return parser.parse_args()


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def load_auth_headers(auth_json_path: str, user_agent: str) -> dict[str, str]:
    path = Path(auth_json_path).expanduser()
    try:
        payload = json.loads(path.read_text())
    except FileNotFoundError as exc:
        raise SystemExit(f"auth file not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise SystemExit(f"auth file is not valid JSON: {path}: {exc}") from exc

    tokens = payload.get("tokens")
    if not isinstance(tokens, dict):
        raise SystemExit(f"auth file does not contain tokens object: {path}")

    access_token = tokens.get("access_token")
    if not isinstance(access_token, str) or not access_token.strip():
        raise SystemExit(
            "auth file does not contain tokens.access_token. "
            "This probe requires ChatGPT token auth from ~/.codex/auth.json."
        )

    headers = {
        "Authorization": f"Bearer {access_token}",
        "Accept": "application/json",
        "originator": "codex_cli_rs",
        "User-Agent": user_agent,
    }

    account_id = tokens.get("account_id")
    if isinstance(account_id, str) and account_id.strip():
        headers["ChatGPT-Account-ID"] = account_id

    return headers


def join_url(base_url: str, path: str) -> str:
    base_url = base_url.rstrip("/")
    if not path.startswith("/"):
        path = f"/{path}"
    return f"{base_url}{path}"


def response_request_id(headers: Any) -> str:
    for name in (
        "x-request-id",
        "openai-request-id",
        "cf-ray",
        "x-envoy-upstream-service-time",
    ):
        value = headers.get(name)
        if value:
            return str(value)
    return ""


def perform_request(
    endpoint: ProbeEndpoint,
    base_url: str,
    headers: dict[str, str],
    iteration: int,
    order: int,
) -> dict[str, Any]:
    url = join_url(base_url, endpoint.path)
    request = urllib.request.Request(url, headers=headers, method="GET")
    started_at = utc_now_iso()
    started_ns = time.perf_counter_ns()
    status = ""
    response_bytes = 0
    request_id = ""
    error = ""

    try:
        with urllib.request.urlopen(request, timeout=endpoint.timeout_sec) as response:
            body = response.read()
            status = str(response.status)
            response_bytes = len(body)
            request_id = response_request_id(response.headers)
    except urllib.error.HTTPError as exc:
        body = exc.read()
        status = str(exc.code)
        response_bytes = len(body)
        request_id = response_request_id(exc.headers)
        error = f"HTTPError: {exc.code} {exc.reason}"
    except urllib.error.URLError as exc:
        error = f"URLError: {exc.reason}"
    except TimeoutError as exc:
        error = f"TimeoutError: {exc}"
    except Exception as exc:  # Keep the long-running probe alive across one bad call.
        error = f"{type(exc).__name__}: {exc}"

    ended_ns = time.perf_counter_ns()
    ended_at = utc_now_iso()
    latency_ns = ended_ns - started_ns
    latency_ms = latency_ns / 1_000_000
    success = status.startswith("2") and not error

    return {
        "iteration": iteration,
        "order": order,
        "label": endpoint.api_label(),
        "method": "GET",
        "url_path_query": endpoint.path,
        "started_at": started_at,
        "ended_at": ended_at,
        "latency_ms": f"{latency_ms:.3f}",
        "latency_ns": latency_ns,
        "status": status,
        "success": str(success).lower(),
        "response_bytes": response_bytes,
        "request_id": request_id,
        "error": error,
    }


def nearest_rank(values: list[float], percentile: float) -> float | None:
    if not values:
        return None
    sorted_values = sorted(values)
    rank = max(1, math.ceil((percentile / 100) * len(sorted_values)))
    return sorted_values[rank - 1]


def summarize_rows(rows: list[dict[str, Any]]) -> dict[str, Any]:
    summaries: dict[str, Any] = {}
    for endpoint in ENDPOINTS:
        api_label = endpoint.api_label()
        endpoint_rows = [row for row in rows if row["label"] == api_label]
        latencies = [float(row["latency_ms"]) for row in endpoint_rows]
        success_rows = [row for row in endpoint_rows if row["success"] == "true"]
        error_rows = [row for row in endpoint_rows if row["success"] != "true"]
        non_2xx_rows = [
            row
            for row in endpoint_rows
            if not str(row["status"]).startswith("2")
        ]

        if latencies:
            summary = {
                "count": len(endpoint_rows),
                "success_count": len(success_rows),
                "error_count": len(error_rows),
                "non_2xx_count": len(non_2xx_rows),
                "min_ms": min(latencies),
                "mean_ms": statistics.fmean(latencies),
                "p50_ms": statistics.median(latencies),
                "p90_ms": nearest_rank(latencies, 90),
                "p95_ms": nearest_rank(latencies, 95),
                "p99_ms": nearest_rank(latencies, 99),
                "max_ms": max(latencies),
                "stdev_ms": statistics.stdev(latencies) if len(latencies) > 1 else 0.0,
            }
        else:
            summary = {
                "count": 0,
                "success_count": 0,
                "error_count": 0,
                "non_2xx_count": 0,
                "min_ms": None,
                "mean_ms": None,
                "p50_ms": None,
                "p90_ms": None,
                "p95_ms": None,
                "p99_ms": None,
                "max_ms": None,
                "stdev_ms": None,
            }

        summaries[api_label] = summary

    return {
        "generated_at": utc_now_iso(),
        "percentile_method": "nearest_rank",
        "endpoints": summaries,
    }


def write_outputs(
    rows: list[dict[str, Any]],
    csv_path: Path,
    summary_path: Path,
) -> dict[str, Any]:
    with csv_path.open("w", newline="") as file:
        writer = csv.DictWriter(file, fieldnames=CSV_FIELDS)
        writer.writeheader()
        writer.writerows(rows)

    summary = summarize_rows(rows)
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    return summary


def print_summary(summary: dict[str, Any]) -> None:
    print()
    print("Latency summary, milliseconds")
    print(
        "label,count,success,error,non_2xx,min,mean,p50,p90,p95,p99,max,stdev"
    )
    for endpoint in ENDPOINTS:
        api_label = endpoint.api_label()
        item = summary["endpoints"][api_label]
        values = [
            api_label,
            item["count"],
            item["success_count"],
            item["error_count"],
            item["non_2xx_count"],
            item["min_ms"],
            item["mean_ms"],
            item["p50_ms"],
            item["p90_ms"],
            item["p95_ms"],
            item["p99_ms"],
            item["max_ms"],
            item["stdev_ms"],
        ]
        print(
            ",".join(
                f"{value:.3f}" if isinstance(value, float) else str(value)
                for value in values
            )
        )


def validate_args(args: argparse.Namespace) -> None:
    if args.iterations < 1:
        raise SystemExit("--iterations must be >= 1")
    if args.call_gap_sec < 0:
        raise SystemExit("--call-gap-sec must be >= 0")
    if args.iteration_gap_sec < 0:
        raise SystemExit("--iteration-gap-sec must be >= 0")


def main() -> int:
    args = parse_args()
    validate_args(args)
    headers = load_auth_headers(args.auth_json, args.user_agent)

    output_dir = Path(args.output_dir).expanduser()
    output_dir.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    csv_path = output_dir / f"codex-plugin-api-latency-{stamp}.csv"
    summary_path = output_dir / f"codex-plugin-api-latency-{stamp}.summary.json"

    rows: list[dict[str, Any]] = []
    total_calls = args.iterations * len(ENDPOINTS)
    completed_calls = 0

    print(f"base_url={args.base_url.rstrip('/')}")
    print(f"iterations={args.iterations}")
    print(f"call_gap_sec={args.call_gap_sec}")
    print(f"iteration_gap_sec={args.iteration_gap_sec}")
    print(f"csv_path={csv_path}")
    print(f"summary_path={summary_path}")
    print(f"total_calls={total_calls}")
    print()

    try:
        for iteration in range(1, args.iterations + 1):
            for order, endpoint in enumerate(ENDPOINTS, start=1):
                row = perform_request(
                    endpoint=endpoint,
                    base_url=args.base_url,
                    headers=headers,
                    iteration=iteration,
                    order=order,
                )
                rows.append(row)
                completed_calls += 1
                status = row["status"] or "no-status"
                outcome = "ok" if row["success"] == "true" else "error"
                print(
                    f"[{completed_calls}/{total_calls}] "
                    f"iteration={iteration} order={order} label={row['label']} "
                    f"status={status} outcome={outcome} "
                    f"latency_ms={row['latency_ms']}"
                )
                sys.stdout.flush()

                if order != len(ENDPOINTS) and args.call_gap_sec > 0:
                    time.sleep(args.call_gap_sec)

            if iteration != args.iterations and args.iteration_gap_sec > 0:
                time.sleep(args.iteration_gap_sec)
    except KeyboardInterrupt:
        print()
        print("Interrupted; writing partial outputs.")

    summary = write_outputs(rows, csv_path, summary_path)
    print_summary(summary)
    print()
    print(f"Wrote CSV: {csv_path}")
    print(f"Wrote summary: {summary_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
