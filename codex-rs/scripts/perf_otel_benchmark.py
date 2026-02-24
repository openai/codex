#!/usr/bin/env python3
"""Run reproducible Codex perf benchmarks with local OTEL capture."""

from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import math
import os
import platform
import re
import shlex
import shutil
import signal
import statistics
import subprocess
import tempfile
import textwrap
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
import xml.etree.ElementTree as ET

QUEUE_CANCEL_RE = re.compile(r"queue|queued|cancel|cancell|interrupt|abort")
TURN_LATENCY_RE = re.compile(r"turn.*(duration|latency)|codex\.turn", re.IGNORECASE)
ACTION_LATENCY_RE = re.compile(
    r"(tool|exec_command|apply_patch|search|agent).*(duration|latency)|codex\.tool",
    re.IGNORECASE,
)
STREAM_LATENCY_RE = re.compile(
    r"(stream|first_token|first_event|ttfb|chunk).*(duration|latency|ms)",
    re.IGNORECASE,
)


@dataclass
class WorkerResult:
    worker_id: int
    return_code: int
    duration_ms: float
    max_rss_kb: int | None
    user_cpu_sec: float | None
    system_cpu_sec: float | None
    cpu_pct: float | None
    voluntary_ctx_switches: int | None
    involuntary_ctx_switches: int | None
    peak_open_fds: int | None
    peak_direct_children: int | None
    sample_count: int
    sampled_peak_parent_rss_kb: int | None
    sampled_peak_parent_cpu_pct: float | None
    sampled_peak_tree_rss_kb: int | None
    sampled_peak_tree_cpu_pct: float | None
    sampled_mean_tree_cpu_pct: float | None
    build_cmd_ms: float
    spawn_proc_ms: float
    monitor_loop_ms: float
    communicate_ms: float
    parse_stats_ms: float
    top_sample_count: int
    top_peak_rss_mb: float | None
    top_mean_rss_mb: float | None
    top_peak_cpu_pct: float | None
    top_mean_cpu_pct: float | None
    vmmap_start_physical_footprint_mb: float | None
    vmmap_mid_physical_footprint_mb: float | None
    vmmap_end_physical_footprint_mb: float | None
    xctrace_trace_path: str | None
    xctrace_hotspots: list[dict[str, Any]] | None
    stderr_tail: str


@dataclass
class IterationResult:
    iteration: int
    duration_ms: float
    throughput_runs_per_sec: float
    max_rss_kb: int | None
    user_cpu_sec: float | None
    system_cpu_sec: float | None
    cpu_pct: float | None
    voluntary_ctx_switches: int | None
    involuntary_ctx_switches: int | None
    peak_open_fds: int | None
    peak_direct_children: int | None
    return_code: int
    worker_count: int
    successful_runs: int
    failed_runs: int
    otel_payload_count: int
    metric_datapoint_count: int
    queue_cancel_datapoints: dict[str, int]
    turn_metric_points: int
    action_metric_points: int
    stream_metric_points: int
    turn_metric_value_sum: float | None
    action_metric_value_sum: float | None
    stream_metric_value_sum: float | None
    sampled_peak_tree_rss_kb: int | None
    sampled_peak_tree_cpu_pct: float | None
    sampled_mean_tree_cpu_pct: float | None
    build_cmd_ms: float | None
    spawn_proc_ms: float | None
    monitor_loop_ms: float | None
    communicate_ms: float | None
    parse_stats_ms: float | None
    top_sample_count: int | None
    top_peak_rss_mb: float | None
    top_mean_rss_mb: float | None
    top_peak_cpu_pct: float | None
    top_mean_cpu_pct: float | None
    vmmap_start_physical_footprint_mb: float | None
    vmmap_mid_physical_footprint_mb: float | None
    vmmap_end_physical_footprint_mb: float | None
    xctrace_trace_path: str | None
    xctrace_hotspots: list[dict[str, Any]] | None
    stderr_tail: str
    worker_results: list[WorkerResult] | None = None


class _CollectorState:
    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._records: list[dict[str, Any]] = []

    def append(self, record: dict[str, Any]) -> None:
        with self._lock:
            self._records.append(record)

    def clear(self) -> None:
        with self._lock:
            self._records.clear()

    def snapshot(self) -> list[dict[str, Any]]:
        with self._lock:
            return list(self._records)


class CollectorHandler(BaseHTTPRequestHandler):
    state: _CollectorState

    def do_POST(self) -> None:  # noqa: N802
        length = int(self.headers.get("content-length", "0"))
        raw = self.rfile.read(length)
        parsed: Any = None
        text = raw.decode("utf-8", errors="replace")
        try:
            parsed = json.loads(text)
        except json.JSONDecodeError:
            parsed = {"raw": text}

        self.state.append(
            {
                "timestamp": dt.datetime.now(dt.timezone.utc).isoformat(),
                "path": self.path,
                "headers": {k.lower(): v for k, v in self.headers.items()},
                "body": parsed,
            }
        )

        self.send_response(200)
        self.end_headers()
        self.wfile.write(b"ok")

    def log_message(self, _format: str, *_args: Any) -> None:
        return


def start_collector() -> tuple[ThreadingHTTPServer, _CollectorState, threading.Thread]:
    state = _CollectorState()

    class Handler(CollectorHandler):
        pass

    Handler.state = state
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, state, thread


def metric_points(payload: Any) -> list[dict[str, Any]]:
    points: list[dict[str, Any]] = []
    if not isinstance(payload, dict):
        return points

    for rm in payload.get("resourceMetrics", []):
        for sm in rm.get("scopeMetrics", []):
            for metric in sm.get("metrics", []):
                name = str(metric.get("name", ""))
                points.extend(_extract_points(metric, name))
    return points


def _extract_points(metric: dict[str, Any], name: str) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for kind in ("sum", "gauge", "histogram", "exponentialHistogram"):
        payload = metric.get(kind)
        if not isinstance(payload, dict):
            continue
        for dp in payload.get("dataPoints", []):
            attrs = _parse_attrs(dp)
            count = int(dp.get("count", 1) or 1)
            if kind in ("histogram", "exponentialHistogram"):
                value = _to_float(dp.get("sum"))
            else:
                value = _to_float(dp.get("asDouble"))
                if value is None:
                    value = _to_float(dp.get("asInt"))

            out.append(
                {
                    "name": name,
                    "kind": kind,
                    "count": count,
                    "value": value,
                    "attrs": attrs,
                }
            )
    return out


def _parse_attrs(datapoint: dict[str, Any]) -> dict[str, str]:
    attrs: dict[str, str] = {}
    for item in datapoint.get("attributes", []):
        key = item.get("key")
        if not isinstance(key, str):
            continue
        value = item.get("value", {})
        attrs[key] = _any_value_to_string(value)
    return attrs


def _any_value_to_string(value: Any) -> str:
    if isinstance(value, dict):
        for key in ("stringValue", "intValue", "doubleValue", "boolValue"):
            if key in value:
                return str(value[key])
        return json.dumps(value, sort_keys=True)
    return str(value)


def _to_float(value: Any) -> float | None:
    if value is None:
        return None
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        try:
            return float(value)
        except ValueError:
            return None
    return None


def parse_max_rss_kb(stderr: str) -> int | None:
    darwin_match = re.search(r"^(\d+)\s+maximum resident set size$", stderr, flags=re.MULTILINE)
    if darwin_match:
        return int(darwin_match.group(1))

    # Newer macOS /usr/bin/time output labels this metric as peak memory footprint.
    darwin_peak_match = re.search(r"^\s*(\d+)\s+peak memory footprint$", stderr, flags=re.MULTILINE)
    if darwin_peak_match:
        bytes_value = int(darwin_peak_match.group(1))
        return math.ceil(bytes_value / 1024.0)

    linux_match = re.search(r"MAXRSS_KB=(\d+)", stderr)
    if linux_match:
        return int(linux_match.group(1))

    return None


def _parse_first_float(stderr: str, patterns: list[str]) -> float | None:
    for pattern in patterns:
        match = re.search(pattern, stderr, flags=re.MULTILINE)
        if match:
            try:
                return float(match.group(1))
            except ValueError:
                return None
    return None


def _parse_percent_float(stderr: str, patterns: list[str]) -> float | None:
    for pattern in patterns:
        match = re.search(pattern, stderr, flags=re.MULTILINE)
        if not match:
            continue
        raw = match.group(1).strip()
        if raw.endswith("%"):
            raw = raw[:-1]
        try:
            return float(raw)
        except ValueError:
            return None
    return None


def _parse_first_int(stderr: str, patterns: list[str]) -> int | None:
    for pattern in patterns:
        match = re.search(pattern, stderr, flags=re.MULTILINE)
        if match:
            try:
                return int(match.group(1))
            except ValueError:
                return None
    return None


def parse_cpu_stats(stderr: str) -> tuple[float | None, float | None, float | None]:
    user_cpu_sec = _parse_first_float(
        stderr,
        [r"USER_SEC=([0-9.]+)", r"([0-9.]+)\s+user(?:\s|$)"],
    )
    system_cpu_sec = _parse_first_float(
        stderr,
        [r"SYS_SEC=([0-9.]+)", r"([0-9.]+)\s+(?:sys|system)(?:\s|$)"],
    )
    cpu_pct = _parse_percent_float(
        stderr,
        [r"CPU_PCT=([0-9.]+%?)", r"^\s*([0-9.]+%?)\s+percent cpu\s*$"],
    )
    return user_cpu_sec, system_cpu_sec, cpu_pct


def parse_context_switches(stderr: str) -> tuple[int | None, int | None]:
    voluntary = _parse_first_int(
        stderr,
        [r"VOL_CTX_SWITCHES=(\d+)", r"^\s*(\d+)\s+voluntary context switches\s*$"],
    )
    involuntary = _parse_first_int(
        stderr,
        [r"INVOL_CTX_SWITCHES=(\d+)", r"^\s*(\d+)\s+involuntary context switches\s*$"],
    )
    return voluntary, involuntary


def _sample_open_fds(pid: int) -> int | None:
    proc_fd_dir = Path(f"/proc/{pid}/fd")
    if proc_fd_dir.exists():
        try:
            return len(list(proc_fd_dir.iterdir()))
        except OSError:
            return None

    ps_bin = shutil.which("ps")
    if not ps_bin:
        return None

    proc = subprocess.run(
        [ps_bin, "-o", "nfiles=", "-p", str(pid)],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode == 0:
        text = proc.stdout.strip()
        if text:
            try:
                return int(text.splitlines()[0].strip())
            except ValueError:
                pass

    lsof_bin = shutil.which("lsof")
    if not lsof_bin:
        return None
    lsof_proc = subprocess.run(
        [lsof_bin, "-n", "-p", str(pid)],
        capture_output=True,
        text=True,
        check=False,
    )
    if lsof_proc.returncode != 0:
        return None
    lines = [line for line in lsof_proc.stdout.splitlines() if line.strip()]
    if not lines:
        return None
    return max(0, len(lines) - 1)


def _sample_direct_children(pid: int) -> int | None:
    child_pids = _list_direct_child_pids(pid)
    if child_pids is None:
        return None
    return len(child_pids)


def _list_direct_child_pids(pid: int) -> list[int] | None:
    pgrep_bin = shutil.which("pgrep")
    if not pgrep_bin:
        return None
    proc = subprocess.run(
        [pgrep_bin, "-P", str(pid)],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode == 1:
        return []
    if proc.returncode != 0:
        return None
    out: list[int] = []
    for line in proc.stdout.splitlines():
        value = line.strip()
        if not value:
            continue
        try:
            out.append(int(value))
        except ValueError:
            continue
    return out


def _sample_proc_rss_cpu(pid: int) -> tuple[int | None, float | None]:
    ps_bin = shutil.which("ps")
    if not ps_bin:
        return None, None
    proc = subprocess.run(
        [ps_bin, "-o", "rss=,%cpu=", "-p", str(pid)],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        return None, None
    line = proc.stdout.strip().splitlines()
    if not line:
        return None, None
    parts = [part for part in line[0].strip().split() if part]
    if len(parts) < 2:
        return None, None
    try:
        rss_kb = int(float(parts[0]))
        cpu_pct = float(parts[1])
        return rss_kb, cpu_pct
    except ValueError:
        return None, None


def _sample_children_rss_cpu(pid: int) -> tuple[int | None, float | None]:
    pgrep_bin = shutil.which("pgrep")
    ps_bin = shutil.which("ps")
    if not pgrep_bin or not ps_bin:
        return None, None
    proc = subprocess.run(
        [pgrep_bin, "-P", str(pid)],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode == 1:
        return None, None
    if proc.returncode != 0:
        return None, None
    child_pids = [line.strip() for line in proc.stdout.splitlines() if line.strip()]
    if not child_pids:
        return None, None
    ps_proc = subprocess.run(
        [ps_bin, "-o", "rss=,%cpu=", "-p", ",".join(child_pids)],
        capture_output=True,
        text=True,
        check=False,
    )
    if ps_proc.returncode != 0:
        return None, None
    rss_total = 0
    cpu_total = 0.0
    for line in ps_proc.stdout.splitlines():
        parts = [part for part in line.strip().split() if part]
        if len(parts) < 2:
            continue
        try:
            rss_total += int(float(parts[0]))
            cpu_total += float(parts[1])
        except ValueError:
            continue
    return rss_total, cpu_total


def _parse_top_mem_mb(raw: str) -> float | None:
    token = raw.strip().upper().rstrip("+")
    if not token:
        return None
    match = re.match(r"^([0-9]+(?:\.[0-9]+)?)([BKMGTP]?)$", token)
    if not match:
        return None
    value = float(match.group(1))
    unit = match.group(2) or "M"
    if unit == "B":
        return value / (1024.0 * 1024.0)
    if unit == "K":
        return value / 1024.0
    if unit == "M":
        return value
    if unit == "G":
        return value * 1024.0
    if unit == "T":
        return value * 1024.0 * 1024.0
    if unit == "P":
        return value * 1024.0 * 1024.0 * 1024.0
    return None


def _sample_top_pid(pid: int) -> tuple[float | None, float | None]:
    rss_kb, cpu_pct = _sample_proc_rss_cpu(pid)
    if rss_kb is not None or cpu_pct is not None:
        mem_mb = (rss_kb / 1024.0) if rss_kb is not None else None
        return mem_mb, cpu_pct

    top_bin = shutil.which("top")
    if not top_bin:
        return None, None
    proc = subprocess.run(
        [top_bin, "-l", "1", "-pid", str(pid), "-stats", "pid,cpu,mem"],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        return None, None
    for line in proc.stdout.splitlines():
        parts = [part for part in line.strip().split() if part]
        if len(parts) < 3:
            continue
        if parts[0] != str(pid):
            continue
        try:
            cpu_pct = float(parts[1].rstrip("%"))
        except ValueError:
            cpu_pct = None
        mem_mb = _parse_top_mem_mb(parts[2])
        return mem_mb, cpu_pct
    return None, None


def _parse_binary_size_to_mb(text: str) -> float | None:
    token = text.strip().upper().rstrip("+")
    if not token:
        return None
    match = re.match(r"^([0-9]+(?:\.[0-9]+)?)([BKMGTPE]?)$", token)
    if not match:
        return None
    value = float(match.group(1))
    unit = match.group(2) or "B"
    multipliers = {
        "B": 1.0 / (1024.0 * 1024.0),
        "K": 1.0 / 1024.0,
        "M": 1.0,
        "G": 1024.0,
        "T": 1024.0 * 1024.0,
        "P": 1024.0 * 1024.0 * 1024.0,
        "E": 1024.0 * 1024.0 * 1024.0 * 1024.0,
    }
    return value * multipliers[unit]


def _select_target_pid(parent_pid: int) -> int:
    direct_children = _list_direct_child_pids(parent_pid)
    if direct_children:
        return direct_children[0]
    return parent_pid


def _capture_vmmap_snapshot(pid: int) -> dict[str, float | None] | None:
    vmmap_bin = shutil.which("vmmap")
    if not vmmap_bin:
        return None
    proc = subprocess.run(
        [vmmap_bin, "-summary", str(pid)],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        return None
    physical = None
    physical_peak = None
    resident = None
    physical_match = re.search(r"^Physical footprint:\s*([0-9]+(?:\.[0-9]+)?[BKMGTPE]?)\s*$", proc.stdout, re.MULTILINE)
    if physical_match:
        physical = _parse_binary_size_to_mb(physical_match.group(1))
    physical_peak_match = re.search(
        r"^Physical footprint \(peak\):\s*([0-9]+(?:\.[0-9]+)?[BKMGTPE]?)\s*$",
        proc.stdout,
        re.MULTILINE,
    )
    if physical_peak_match:
        physical_peak = _parse_binary_size_to_mb(physical_peak_match.group(1))
    total_match = re.search(
        r"^TOTAL(?:, minus reserved VM space)?\s+\S+\s+([0-9]+(?:\.[0-9]+)?[BKMGTPE]?)\b",
        proc.stdout,
        re.MULTILINE,
    )
    if total_match:
        resident = _parse_binary_size_to_mb(total_match.group(1))
    return {
        "physical_footprint_mb": physical,
        "physical_footprint_peak_mb": physical_peak,
        "resident_mb": resident,
    }


def _extract_xctrace_hotspots(trace_path: Path, limit: int) -> list[dict[str, Any]]:
    xctrace_bin = shutil.which("xctrace")
    if not xctrace_bin:
        return []
    xml_path = trace_path.with_suffix(".time-profile.xml")
    export_proc = subprocess.run(
        [
            xctrace_bin,
            "export",
            "--input",
            str(trace_path),
            "--xpath",
            "/trace-toc/run[@number=\"1\"]/data/table[@schema=\"time-profile\"]",
            "--output",
            str(xml_path),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if export_proc.returncode != 0 or not xml_path.exists():
        return []
    try:
        root = ET.parse(xml_path).getroot()
    except ET.ParseError:
        return []
    by_frame: dict[str, dict[str, float]] = {}
    for row in root.findall(".//row"):
        backtrace = row.find("backtrace")
        if backtrace is None:
            continue
        frame = backtrace.find("frame")
        if frame is None:
            continue
        frame_name = html.unescape(frame.attrib.get("name", "<unknown>"))
        weight_text = row.findtext("weight")
        try:
            weight_ns = float(weight_text) if weight_text else 1_000_000.0
        except ValueError:
            weight_ns = 1_000_000.0
        if frame_name not in by_frame:
            by_frame[frame_name] = {"weight_ns": 0.0, "samples": 0.0}
        by_frame[frame_name]["weight_ns"] += weight_ns
        by_frame[frame_name]["samples"] += 1.0
    ranked = sorted(by_frame.items(), key=lambda item: item[1]["weight_ns"], reverse=True)
    hotspots: list[dict[str, Any]] = []
    for frame_name, payload in ranked[: max(1, limit)]:
        hotspots.append(
            {
                "frame": frame_name,
                "weight_ms": payload["weight_ns"] / 1_000_000.0,
                "samples": int(payload["samples"]),
            }
        )
    return hotspots


def percentile(values: list[float], pct: float) -> float:
    if not values:
        return math.nan
    if len(values) == 1:
        return values[0]
    sorted_values = sorted(values)
    idx = pct * (len(sorted_values) - 1)
    lo = math.floor(idx)
    hi = math.ceil(idx)
    if lo == hi:
        return sorted_values[lo]
    frac = idx - lo
    return sorted_values[lo] * (1 - frac) + sorted_values[hi] * frac


def write_config(codex_home: Path, endpoint: str) -> Path:
    codex_home.mkdir(parents=True, exist_ok=True)
    config_path = codex_home / "config.toml"
    config = textwrap.dedent(
        f"""
        [analytics]
        enabled = true

        [otel]
        environment = "perf-local"
        exporter = "none"
        trace_exporter = "none"
        metrics_exporter = {{ otlp-http = {{ endpoint = "{endpoint}", protocol = "json" }} }}
        """
    ).strip()
    config_path.write_text(config + "\n", encoding="utf-8")
    return config_path


def resolve_codex_home(path_override: str | None) -> Path:
    if path_override:
        return Path(path_override).expanduser()
    env_home = os.environ.get("CODEX_HOME")
    if env_home:
        return Path(env_home).expanduser()
    return Path.home() / ".codex"


def copy_account_auth(src_home: Path, dst_home: Path) -> bool:
    # Copy auth.json for account-auth runs
    src_auth = src_home / "auth.json"
    dst_auth = dst_home / "auth.json"
    if not src_auth.exists():
        return False
    dst_home.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src_auth, dst_auth)
    
    # Also copy config.toml for custom model providers/profiles
    src_config = src_home / "config.toml"
    dst_config = dst_home / "config.toml"
    if src_config.exists():
        shutil.copy2(src_config, dst_config)
    
    return True


def build_command(cmd: str) -> list[str]:
    cmd_parts = shlex.split(cmd)
    if not cmd_parts:
        raise ValueError("--cmd resolved to an empty command")

    # Skip time wrapper for exec commands - they have their own timing/RSS reporting
    # and the time wrapper can cause return code detection issues
    if "exec" in cmd:
        return cmd_parts

    time_bin = shutil.which("/usr/bin/time") or shutil.which("time")
    if not time_bin:
        return cmd_parts

    if platform.system().lower() == "darwin":
        return [time_bin, "-l", *cmd_parts]

    time_format = "\n".join(
        [
            "MAXRSS_KB=%M",
            "USER_SEC=%U",
            "SYS_SEC=%S",
            "CPU_PCT=%P",
            "VOL_CTX_SWITCHES=%w",
            "INVOL_CTX_SWITCHES=%c",
        ]
    )
    return [time_bin, "-f", time_format, *cmd_parts]


def _run_worker(
    worker_id: int,
    cmd: str,
    env: dict[str, str],
    timeout_sec: float,
    enable_top_attach: bool,
    top_interval_sec: float,
    enable_vmmap_snapshots: bool,
    enable_xctrace_capture: bool,
    xctrace_time_limit_sec: float,
    xctrace_hotspots_limit: int,
    xctrace_artifact_dir: Path | None,
    monitor_sleep_sec: float,
    probe_interval_sec: float,
) -> WorkerResult:
    build_start = time.perf_counter()
    trace_path: Path | None = None
    if enable_xctrace_capture:
        cmd_parts = shlex.split(cmd)
        if not cmd_parts:
            raise ValueError("--cmd resolved to an empty command")
        resolved_bin = shutil.which(cmd_parts[0])
        if resolved_bin:
            cmd_parts[0] = resolved_bin
        xctrace_bin = shutil.which("xctrace")
        if xctrace_bin and xctrace_artifact_dir is not None:
            xctrace_artifact_dir.mkdir(parents=True, exist_ok=True)
            trace_path = xctrace_artifact_dir / f"worker-{worker_id:02d}.trace"
            full_cmd = [
                xctrace_bin,
                "record",
                "--template",
                "Time Profiler",
                "--output",
                str(trace_path),
                "--time-limit",
                f"{max(1, int(math.ceil(xctrace_time_limit_sec)))}s",
                "--no-prompt",
                "--launch",
                "--",
                *cmd_parts,
            ]
        else:
            full_cmd = build_command(cmd)
    else:
        full_cmd = build_command(cmd)
    build_cmd_ms = (time.perf_counter() - build_start) * 1000.0

    start = time.perf_counter()
    return_code = 124
    stderr_text = "command timed out"
    stdout_text = ""
    peak_open_fds: int | None = None
    peak_direct_children: int | None = None
    sample_count = 0
    peak_parent_rss_kb: int | None = None
    peak_parent_cpu_pct: float | None = None
    peak_tree_rss_kb: int | None = None
    peak_tree_cpu_pct: float | None = None
    tree_cpu_samples: list[float] = []
    top_mem_samples_mb: list[float] = []
    top_cpu_samples_pct: list[float] = []
    vmmap_start_snapshot: dict[str, float | None] | None = None
    vmmap_mid_snapshot: dict[str, float | None] | None = None
    vmmap_end_snapshot: dict[str, float | None] | None = None
    monitor_loop_ms = 0.0

    spawn_start = time.perf_counter()
    proc = subprocess.Popen(
        full_cmd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    spawn_proc_ms = (time.perf_counter() - spawn_start) * 1000.0
    monitor_start = time.monotonic()
    vmmap_mid_due_sec = min(max(timeout_sec / 2.0, 0.2), 1.0)
    if enable_vmmap_snapshots:
        vmmap_start_snapshot = _capture_vmmap_snapshot(_select_target_pid(proc.pid))
    if proc.poll() is None:
        initial_fds = _sample_open_fds(proc.pid)
        if initial_fds is not None:
            peak_open_fds = initial_fds
        initial_children = _sample_direct_children(proc.pid)
        if initial_children is not None:
            peak_direct_children = initial_children
        initial_parent_rss_kb, initial_parent_cpu_pct = _sample_proc_rss_cpu(proc.pid)
        initial_child_rss_kb, initial_child_cpu_pct = _sample_children_rss_cpu(proc.pid)
        if enable_top_attach:
            if initial_parent_rss_kb is not None or initial_child_rss_kb is not None:
                top_mem_samples_mb.append(((initial_parent_rss_kb or 0) + (initial_child_rss_kb or 0)) / 1024.0)
            if initial_parent_cpu_pct is not None or initial_child_cpu_pct is not None:
                top_cpu_samples_pct.append((initial_parent_cpu_pct or 0.0) + (initial_child_cpu_pct or 0.0))

    deadline = time.monotonic() + timeout_sec
    next_top_sample = time.monotonic()
    next_probe_sample = time.monotonic()
    timed_out = False
    while proc.poll() is None:
        loop_start = time.perf_counter()
        now = time.monotonic()

        if now >= next_probe_sample:
            sampled_fds = _sample_open_fds(proc.pid)
            if sampled_fds is not None:
                peak_open_fds = sampled_fds if peak_open_fds is None else max(peak_open_fds, sampled_fds)

            sampled_children = _sample_direct_children(proc.pid)
            if sampled_children is not None:
                peak_direct_children = (
                    sampled_children
                    if peak_direct_children is None
                    else max(peak_direct_children, sampled_children)
                )

            parent_rss_kb, parent_cpu_pct = _sample_proc_rss_cpu(proc.pid)
            if parent_rss_kb is not None:
                peak_parent_rss_kb = (
                    parent_rss_kb if peak_parent_rss_kb is None else max(peak_parent_rss_kb, parent_rss_kb)
                )
            if parent_cpu_pct is not None:
                peak_parent_cpu_pct = (
                    parent_cpu_pct if peak_parent_cpu_pct is None else max(peak_parent_cpu_pct, parent_cpu_pct)
                )

            child_rss_kb, child_cpu_pct = _sample_children_rss_cpu(proc.pid)
            if parent_rss_kb is not None or child_rss_kb is not None:
                tree_rss = (parent_rss_kb or 0) + (child_rss_kb or 0)
                peak_tree_rss_kb = tree_rss if peak_tree_rss_kb is None else max(peak_tree_rss_kb, tree_rss)
            if parent_cpu_pct is not None or child_cpu_pct is not None:
                tree_cpu = (parent_cpu_pct or 0.0) + (child_cpu_pct or 0.0)
                tree_cpu_samples.append(tree_cpu)
                peak_tree_cpu_pct = tree_cpu if peak_tree_cpu_pct is None else max(peak_tree_cpu_pct, tree_cpu)
            if enable_top_attach and now >= next_top_sample:
                if parent_rss_kb is not None or child_rss_kb is not None:
                    top_mem_samples_mb.append(((parent_rss_kb or 0) + (child_rss_kb or 0)) / 1024.0)
                if parent_cpu_pct is not None or child_cpu_pct is not None:
                    top_cpu_samples_pct.append((parent_cpu_pct or 0.0) + (child_cpu_pct or 0.0))
                next_top_sample = now + max(top_interval_sec, 0.05)

            if (
                enable_vmmap_snapshots
                and vmmap_mid_snapshot is None
                and (time.monotonic() - monitor_start) >= vmmap_mid_due_sec
            ):
                vmmap_mid_snapshot = _capture_vmmap_snapshot(_select_target_pid(proc.pid))
            sample_count += 1
            next_probe_sample = now + max(probe_interval_sec, 0.05)

        monitor_loop_ms += (time.perf_counter() - loop_start) * 1000.0

        if time.monotonic() >= deadline:
            timed_out = True
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except Exception:
                proc.kill()
            break
        time.sleep(max(monitor_sleep_sec, 0.01))

    if enable_vmmap_snapshots:
        vmmap_end_snapshot = _capture_vmmap_snapshot(_select_target_pid(proc.pid))

    communicate_start = time.perf_counter()
    stdout_text, stderr_text = proc.communicate()
    communicate_ms = (time.perf_counter() - communicate_start) * 1000.0
    if timed_out:
        return_code = 124
        stderr_text = (stderr_text or "") + "\ncommand timed out"
    else:
        return_code = proc.returncode
    _ = stdout_text

    end = time.perf_counter()
    parse_start = time.perf_counter()
    stderr_tail = "\n".join(stderr_text.strip().splitlines()[-5:])
    user_cpu_sec, system_cpu_sec, cpu_pct = parse_cpu_stats(stderr_text)
    voluntary_ctx_switches, involuntary_ctx_switches = parse_context_switches(stderr_text)
    parse_stats_ms = (time.perf_counter() - parse_start) * 1000.0
    xctrace_hotspots = (
        _extract_xctrace_hotspots(trace_path, xctrace_hotspots_limit)
        if trace_path is not None and trace_path.exists()
        else None
    )

    return WorkerResult(
        worker_id=worker_id,
        return_code=return_code,
        duration_ms=(end - start) * 1000.0,
        max_rss_kb=parse_max_rss_kb(stderr_text),
        user_cpu_sec=user_cpu_sec,
        system_cpu_sec=system_cpu_sec,
        cpu_pct=cpu_pct,
        voluntary_ctx_switches=voluntary_ctx_switches,
        involuntary_ctx_switches=involuntary_ctx_switches,
        peak_open_fds=peak_open_fds,
        peak_direct_children=peak_direct_children,
        sample_count=sample_count,
        sampled_peak_parent_rss_kb=peak_parent_rss_kb,
        sampled_peak_parent_cpu_pct=peak_parent_cpu_pct,
        sampled_peak_tree_rss_kb=peak_tree_rss_kb,
        sampled_peak_tree_cpu_pct=peak_tree_cpu_pct,
        sampled_mean_tree_cpu_pct=statistics.fmean(tree_cpu_samples) if tree_cpu_samples else None,
        build_cmd_ms=build_cmd_ms,
        spawn_proc_ms=spawn_proc_ms,
        monitor_loop_ms=monitor_loop_ms,
        communicate_ms=communicate_ms,
        parse_stats_ms=parse_stats_ms,
        top_sample_count=max(len(top_mem_samples_mb), len(top_cpu_samples_pct)),
        top_peak_rss_mb=max(top_mem_samples_mb) if top_mem_samples_mb else None,
        top_mean_rss_mb=statistics.fmean(top_mem_samples_mb) if top_mem_samples_mb else None,
        top_peak_cpu_pct=max(top_cpu_samples_pct) if top_cpu_samples_pct else None,
        top_mean_cpu_pct=statistics.fmean(top_cpu_samples_pct) if top_cpu_samples_pct else None,
        vmmap_start_physical_footprint_mb=(
            vmmap_start_snapshot.get("physical_footprint_mb") if vmmap_start_snapshot else None
        ),
        vmmap_mid_physical_footprint_mb=(
            vmmap_mid_snapshot.get("physical_footprint_mb") if vmmap_mid_snapshot else None
        ),
        vmmap_end_physical_footprint_mb=(
            vmmap_end_snapshot.get("physical_footprint_mb") if vmmap_end_snapshot else None
        ),
        xctrace_trace_path=str(trace_path) if trace_path else None,
        xctrace_hotspots=xctrace_hotspots,
        stderr_tail=stderr_tail,
    )


def _build_worker_envs(base_env: dict[str, str], codex_home_root: Path, concurrency: int) -> list[dict[str, str]]:
    envs: list[dict[str, str]] = []
    for worker_id in range(1, concurrency + 1):
        env = base_env.copy()
        if concurrency == 1:
            worker_home = codex_home_root
        else:
            worker_home = codex_home_root.parent / f"{codex_home_root.name}-worker-{worker_id}"
        env["CODEX_HOME"] = str(worker_home)
        envs.append(env)
    return envs


def summarize_results(
    cmd: str,
    iterations: list[IterationResult],
    run_dir: Path,
    config_path: Path,
    profile: dict[str, Any],
) -> dict[str, Any]:
    durations = [it.duration_ms for it in iterations]
    throughputs = [it.throughput_runs_per_sec for it in iterations]
    rss_values = [float(it.max_rss_kb) for it in iterations if it.max_rss_kb is not None]
    user_cpu_values = [it.user_cpu_sec for it in iterations if it.user_cpu_sec is not None]
    system_cpu_values = [it.system_cpu_sec for it in iterations if it.system_cpu_sec is not None]
    cpu_pct_values = [it.cpu_pct for it in iterations if it.cpu_pct is not None]
    voluntary_ctx_values = [
        float(it.voluntary_ctx_switches)
        for it in iterations
        if it.voluntary_ctx_switches is not None
    ]
    involuntary_ctx_values = [
        float(it.involuntary_ctx_switches)
        for it in iterations
        if it.involuntary_ctx_switches is not None
    ]
    peak_open_fds_values = [float(it.peak_open_fds) for it in iterations if it.peak_open_fds is not None]
    peak_children_values = [
        float(it.peak_direct_children)
        for it in iterations
        if it.peak_direct_children is not None
    ]
    sampled_peak_tree_rss_values = [
        float(it.sampled_peak_tree_rss_kb)
        for it in iterations
        if it.sampled_peak_tree_rss_kb is not None
    ]
    sampled_peak_tree_cpu_values = [
        float(it.sampled_peak_tree_cpu_pct)
        for it in iterations
        if it.sampled_peak_tree_cpu_pct is not None
    ]
    sampled_mean_tree_cpu_values = [
        float(it.sampled_mean_tree_cpu_pct)
        for it in iterations
        if it.sampled_mean_tree_cpu_pct is not None
    ]
    build_cmd_ms_values = [float(it.build_cmd_ms) for it in iterations if it.build_cmd_ms is not None]
    spawn_proc_ms_values = [float(it.spawn_proc_ms) for it in iterations if it.spawn_proc_ms is not None]
    monitor_loop_ms_values = [float(it.monitor_loop_ms) for it in iterations if it.monitor_loop_ms is not None]
    communicate_ms_values = [float(it.communicate_ms) for it in iterations if it.communicate_ms is not None]
    parse_stats_ms_values = [float(it.parse_stats_ms) for it in iterations if it.parse_stats_ms is not None]
    top_sample_count_values = [float(it.top_sample_count) for it in iterations if it.top_sample_count is not None]
    top_peak_rss_mb_values = [float(it.top_peak_rss_mb) for it in iterations if it.top_peak_rss_mb is not None]
    top_mean_rss_mb_values = [float(it.top_mean_rss_mb) for it in iterations if it.top_mean_rss_mb is not None]
    top_peak_cpu_pct_values = [
        float(it.top_peak_cpu_pct) for it in iterations if it.top_peak_cpu_pct is not None
    ]
    top_mean_cpu_pct_values = [
        float(it.top_mean_cpu_pct) for it in iterations if it.top_mean_cpu_pct is not None
    ]
    vmmap_start_physical_values = [
        float(it.vmmap_start_physical_footprint_mb)
        for it in iterations
        if it.vmmap_start_physical_footprint_mb is not None
    ]
    vmmap_mid_physical_values = [
        float(it.vmmap_mid_physical_footprint_mb)
        for it in iterations
        if it.vmmap_mid_physical_footprint_mb is not None
    ]
    vmmap_end_physical_values = [
        float(it.vmmap_end_physical_footprint_mb)
        for it in iterations
        if it.vmmap_end_physical_footprint_mb is not None
    ]
    turn_metric_points_values = [float(it.turn_metric_points) for it in iterations]
    action_metric_points_values = [float(it.action_metric_points) for it in iterations]
    stream_metric_points_values = [float(it.stream_metric_points) for it in iterations]
    turn_metric_value_sums = [it.turn_metric_value_sum for it in iterations if it.turn_metric_value_sum is not None]
    action_metric_value_sums = [
        it.action_metric_value_sum for it in iterations if it.action_metric_value_sum is not None
    ]
    stream_metric_value_sums = [
        it.stream_metric_value_sum for it in iterations if it.stream_metric_value_sum is not None
    ]

    queue_cancel_totals: dict[str, int] = {}
    for it in iterations:
        for name, count in it.queue_cancel_datapoints.items():
            queue_cancel_totals[name] = queue_cancel_totals.get(name, 0) + count

    def maybe_num(value: float) -> float | None:
        return value if math.isfinite(value) else None

    def safe_ratio_pct(numerator: float | None, denominator: float | None) -> float | None:
        if numerator is None or denominator is None or denominator <= 0:
            return None
        return maybe_num((numerator / denominator) * 100.0)

    latency_mean_ms = maybe_num(statistics.fmean(durations) if durations else math.nan)
    build_cmd_mean_ms = maybe_num(statistics.fmean(build_cmd_ms_values) if build_cmd_ms_values else math.nan)
    spawn_proc_mean_ms = maybe_num(statistics.fmean(spawn_proc_ms_values) if spawn_proc_ms_values else math.nan)
    monitor_loop_mean_ms = maybe_num(
        statistics.fmean(monitor_loop_ms_values) if monitor_loop_ms_values else math.nan
    )
    communicate_mean_ms = maybe_num(
        statistics.fmean(communicate_ms_values) if communicate_ms_values else math.nan
    )
    parse_stats_mean_ms = maybe_num(
        statistics.fmean(parse_stats_ms_values) if parse_stats_ms_values else math.nan
    )
    observed_worker_step_mean_ms = None
    if (
        build_cmd_mean_ms is not None
        and spawn_proc_mean_ms is not None
        and monitor_loop_mean_ms is not None
        and communicate_mean_ms is not None
        and parse_stats_mean_ms is not None
    ):
        observed_worker_step_mean_ms = (
            build_cmd_mean_ms
            + spawn_proc_mean_ms
            + monitor_loop_mean_ms
            + communicate_mean_ms
            + parse_stats_mean_ms
        )
    unaccounted_time_mean_ms = None
    if latency_mean_ms is not None and observed_worker_step_mean_ms is not None:
        unaccounted_time_mean_ms = maybe_num(latency_mean_ms - observed_worker_step_mean_ms)

    user_cpu_mean_sec = maybe_num(statistics.fmean(user_cpu_values) if user_cpu_values else math.nan)
    system_cpu_mean_sec = maybe_num(statistics.fmean(system_cpu_values) if system_cpu_values else math.nan)
    total_cpu_mean_sec = None
    if user_cpu_mean_sec is not None and system_cpu_mean_sec is not None:
        total_cpu_mean_sec = maybe_num(user_cpu_mean_sec + system_cpu_mean_sec)

    failed_runs_total = sum(it.failed_runs for it in iterations)
    successful_runs_total = sum(it.successful_runs for it in iterations)
    total_runs = successful_runs_total + failed_runs_total
    timeout_runs_total = sum(1 for it in iterations for w in (it.worker_results or []) if w.return_code == 124)
    if timeout_runs_total == 0:
        timeout_runs_total = sum(1 for it in iterations if it.return_code == 124 and it.worker_count == 1)
    xctrace_traces = [it.xctrace_trace_path for it in iterations if it.xctrace_trace_path]
    hotspot_aggregate: dict[str, dict[str, float]] = {}
    for it in iterations:
        for hotspot in (it.xctrace_hotspots or []):
            frame = hotspot.get("frame")
            if not isinstance(frame, str):
                continue
            weight_ms = float(hotspot.get("weight_ms", 0.0) or 0.0)
            sample_count = float(hotspot.get("samples", 0.0) or 0.0)
            entry = hotspot_aggregate.setdefault(frame, {"weight_ms": 0.0, "samples": 0.0})
            entry["weight_ms"] += weight_ms
            entry["samples"] += sample_count
    hotspot_top = sorted(
        (
            {
                "frame": frame,
                "weight_ms": payload["weight_ms"],
                "samples": int(payload["samples"]),
            }
            for frame, payload in hotspot_aggregate.items()
        ),
        key=lambda item: item["weight_ms"],
        reverse=True,
    )[:10]

    return {
        "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "run_dir": str(run_dir),
        "command": cmd,
        "config_path": str(config_path),
        "profile": profile,
        "iterations": len(iterations),
        "summary": {
            "latency_ms": {
                "mean": latency_mean_ms,
                "p50": maybe_num(percentile(durations, 0.50)),
                "p95": maybe_num(percentile(durations, 0.95)),
                "min": maybe_num(min(durations) if durations else math.nan),
                "max": maybe_num(max(durations) if durations else math.nan),
            },
            "throughput_runs_per_sec": {
                "mean": maybe_num(statistics.fmean(throughputs) if throughputs else math.nan),
                "p50": maybe_num(percentile(throughputs, 0.50)),
                "p95": maybe_num(percentile(throughputs, 0.95)),
            },
            "max_rss_kb": {
                "mean": maybe_num(statistics.fmean(rss_values) if rss_values else math.nan),
                "p50": maybe_num(percentile(rss_values, 0.50) if rss_values else math.nan),
                "p95": maybe_num(percentile(rss_values, 0.95) if rss_values else math.nan),
                "min": maybe_num(min(rss_values) if rss_values else math.nan),
                "max": maybe_num(max(rss_values) if rss_values else math.nan),
            },
            "max_rss_mb": {
                "mean": maybe_num((statistics.fmean(rss_values) / 1024.0) if rss_values else math.nan),
                "p50": maybe_num((percentile(rss_values, 0.50) / 1024.0) if rss_values else math.nan),
                "p95": maybe_num((percentile(rss_values, 0.95) / 1024.0) if rss_values else math.nan),
                "min": maybe_num((min(rss_values) / 1024.0) if rss_values else math.nan),
                "max": maybe_num((max(rss_values) / 1024.0) if rss_values else math.nan),
            },
            "user_cpu_sec": {
                "mean": user_cpu_mean_sec,
                "p50": maybe_num(percentile(user_cpu_values, 0.50) if user_cpu_values else math.nan),
                "p95": maybe_num(percentile(user_cpu_values, 0.95) if user_cpu_values else math.nan),
            },
            "system_cpu_sec": {
                "mean": system_cpu_mean_sec,
                "p50": maybe_num(
                    percentile(system_cpu_values, 0.50) if system_cpu_values else math.nan
                ),
                "p95": maybe_num(
                    percentile(system_cpu_values, 0.95) if system_cpu_values else math.nan
                ),
            },
            "cpu_pct": {
                "mean": maybe_num(statistics.fmean(cpu_pct_values) if cpu_pct_values else math.nan),
                "p50": maybe_num(percentile(cpu_pct_values, 0.50) if cpu_pct_values else math.nan),
                "p95": maybe_num(percentile(cpu_pct_values, 0.95) if cpu_pct_values else math.nan),
            },
            "voluntary_ctx_switches": {
                "mean": maybe_num(
                    statistics.fmean(voluntary_ctx_values) if voluntary_ctx_values else math.nan
                ),
                "p50": maybe_num(
                    percentile(voluntary_ctx_values, 0.50) if voluntary_ctx_values else math.nan
                ),
                "p95": maybe_num(
                    percentile(voluntary_ctx_values, 0.95) if voluntary_ctx_values else math.nan
                ),
            },
            "involuntary_ctx_switches": {
                "mean": maybe_num(
                    statistics.fmean(involuntary_ctx_values) if involuntary_ctx_values else math.nan
                ),
                "p50": maybe_num(
                    percentile(involuntary_ctx_values, 0.50) if involuntary_ctx_values else math.nan
                ),
                "p95": maybe_num(
                    percentile(involuntary_ctx_values, 0.95) if involuntary_ctx_values else math.nan
                ),
            },
            "peak_open_fds": {
                "mean": maybe_num(
                    statistics.fmean(peak_open_fds_values) if peak_open_fds_values else math.nan
                ),
                "p50": maybe_num(
                    percentile(peak_open_fds_values, 0.50) if peak_open_fds_values else math.nan
                ),
                "p95": maybe_num(
                    percentile(peak_open_fds_values, 0.95) if peak_open_fds_values else math.nan
                ),
                "max": maybe_num(max(peak_open_fds_values) if peak_open_fds_values else math.nan),
            },
            "peak_direct_children": {
                "mean": maybe_num(
                    statistics.fmean(peak_children_values) if peak_children_values else math.nan
                ),
                "p50": maybe_num(
                    percentile(peak_children_values, 0.50) if peak_children_values else math.nan
                ),
                "p95": maybe_num(
                    percentile(peak_children_values, 0.95) if peak_children_values else math.nan
                ),
                "max": maybe_num(max(peak_children_values) if peak_children_values else math.nan),
            },
            "process_tree_sampled": {
                "peak_tree_rss_kb": {
                    "mean": maybe_num(
                        statistics.fmean(sampled_peak_tree_rss_values) if sampled_peak_tree_rss_values else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(sampled_peak_tree_rss_values, 0.50)
                        if sampled_peak_tree_rss_values
                        else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(sampled_peak_tree_rss_values, 0.95)
                        if sampled_peak_tree_rss_values
                        else math.nan
                    ),
                    "max": maybe_num(max(sampled_peak_tree_rss_values) if sampled_peak_tree_rss_values else math.nan),
                },
                "peak_tree_rss_mb": {
                    "mean": maybe_num(
                        (statistics.fmean(sampled_peak_tree_rss_values) / 1024.0)
                        if sampled_peak_tree_rss_values
                        else math.nan
                    ),
                    "p50": maybe_num(
                        (percentile(sampled_peak_tree_rss_values, 0.50) / 1024.0)
                        if sampled_peak_tree_rss_values
                        else math.nan
                    ),
                    "p95": maybe_num(
                        (percentile(sampled_peak_tree_rss_values, 0.95) / 1024.0)
                        if sampled_peak_tree_rss_values
                        else math.nan
                    ),
                    "max": maybe_num(
                        (max(sampled_peak_tree_rss_values) / 1024.0)
                        if sampled_peak_tree_rss_values
                        else math.nan
                    ),
                },
                "peak_tree_cpu_pct": {
                    "mean": maybe_num(
                        statistics.fmean(sampled_peak_tree_cpu_values) if sampled_peak_tree_cpu_values else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(sampled_peak_tree_cpu_values, 0.50)
                        if sampled_peak_tree_cpu_values
                        else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(sampled_peak_tree_cpu_values, 0.95)
                        if sampled_peak_tree_cpu_values
                        else math.nan
                    ),
                    "max": maybe_num(max(sampled_peak_tree_cpu_values) if sampled_peak_tree_cpu_values else math.nan),
                },
                "mean_tree_cpu_pct": {
                    "mean": maybe_num(
                        statistics.fmean(sampled_mean_tree_cpu_values) if sampled_mean_tree_cpu_values else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(sampled_mean_tree_cpu_values, 0.50)
                        if sampled_mean_tree_cpu_values
                        else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(sampled_mean_tree_cpu_values, 0.95)
                        if sampled_mean_tree_cpu_values
                        else math.nan
                    ),
                },
            },
            "worker_step_timings_ms": {
                "build_cmd": {
                    "mean": build_cmd_mean_ms,
                    "p50": maybe_num(percentile(build_cmd_ms_values, 0.50) if build_cmd_ms_values else math.nan),
                    "p95": maybe_num(percentile(build_cmd_ms_values, 0.95) if build_cmd_ms_values else math.nan),
                },
                "spawn_proc": {
                    "mean": spawn_proc_mean_ms,
                    "p50": maybe_num(percentile(spawn_proc_ms_values, 0.50) if spawn_proc_ms_values else math.nan),
                    "p95": maybe_num(percentile(spawn_proc_ms_values, 0.95) if spawn_proc_ms_values else math.nan),
                },
                "monitor_loop": {
                    "mean": monitor_loop_mean_ms,
                    "p50": maybe_num(
                        percentile(monitor_loop_ms_values, 0.50) if monitor_loop_ms_values else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(monitor_loop_ms_values, 0.95) if monitor_loop_ms_values else math.nan
                    ),
                },
                "communicate": {
                    "mean": communicate_mean_ms,
                    "p50": maybe_num(
                        percentile(communicate_ms_values, 0.50) if communicate_ms_values else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(communicate_ms_values, 0.95) if communicate_ms_values else math.nan
                    ),
                },
                "parse_stats": {
                    "mean": parse_stats_mean_ms,
                    "p50": maybe_num(
                        percentile(parse_stats_ms_values, 0.50) if parse_stats_ms_values else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(parse_stats_ms_values, 0.95) if parse_stats_ms_values else math.nan
                    ),
                },
            },
            "resource_budget": {
                "time_budget_ms": {
                    "wall_mean_ms": latency_mean_ms,
                    "observed_worker_step_mean_ms": observed_worker_step_mean_ms,
                    "unaccounted_time_mean_ms": unaccounted_time_mean_ms,
                    "build_cmd_share_pct": safe_ratio_pct(build_cmd_mean_ms, latency_mean_ms),
                    "spawn_proc_share_pct": safe_ratio_pct(spawn_proc_mean_ms, latency_mean_ms),
                    "monitor_loop_share_pct": safe_ratio_pct(monitor_loop_mean_ms, latency_mean_ms),
                    "communicate_share_pct": safe_ratio_pct(communicate_mean_ms, latency_mean_ms),
                    "parse_stats_share_pct": safe_ratio_pct(parse_stats_mean_ms, latency_mean_ms),
                    "unaccounted_share_pct": safe_ratio_pct(unaccounted_time_mean_ms, latency_mean_ms),
                },
                "cpu_budget": {
                    "user_cpu_mean_sec": user_cpu_mean_sec,
                    "system_cpu_mean_sec": system_cpu_mean_sec,
                    "total_cpu_mean_sec": total_cpu_mean_sec,
                    "cpu_core_utilization_pct": safe_ratio_pct(
                        total_cpu_mean_sec,
                        (latency_mean_ms / 1000.0) if latency_mean_ms is not None else None,
                    ),
                },
                "process_budget": {
                    "peak_open_fds_mean": maybe_num(
                        statistics.fmean(peak_open_fds_values) if peak_open_fds_values else math.nan
                    ),
                    "peak_direct_children_mean": maybe_num(
                        statistics.fmean(peak_children_values) if peak_children_values else math.nan
                    ),
                    "sampled_peak_tree_rss_mean_kb": maybe_num(
                        statistics.fmean(sampled_peak_tree_rss_values)
                        if sampled_peak_tree_rss_values
                        else math.nan
                    ),
                    "sampled_peak_tree_rss_mean_mb": maybe_num(
                        (statistics.fmean(sampled_peak_tree_rss_values) / 1024.0)
                        if sampled_peak_tree_rss_values
                        else math.nan
                    ),
                    "sampled_peak_tree_cpu_p95": maybe_num(
                        percentile(sampled_peak_tree_cpu_values, 0.95)
                        if sampled_peak_tree_cpu_values
                        else math.nan
                    ),
                },
                "stability": {
                    "total_runs": total_runs,
                    "successful_runs_total": successful_runs_total,
                    "failed_runs_total": failed_runs_total,
                    "timeout_runs_total": timeout_runs_total,
                    "failure_rate_pct": safe_ratio_pct(float(failed_runs_total), float(total_runs)),
                    "timeout_rate_pct": safe_ratio_pct(float(timeout_runs_total), float(total_runs)),
                },
            },
            "top_attach": {
                "sample_count": {
                    "mean": maybe_num(
                        statistics.fmean(top_sample_count_values) if top_sample_count_values else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(top_sample_count_values, 0.50) if top_sample_count_values else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(top_sample_count_values, 0.95) if top_sample_count_values else math.nan
                    ),
                },
                "peak_rss_mb": {
                    "mean": maybe_num(
                        statistics.fmean(top_peak_rss_mb_values) if top_peak_rss_mb_values else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(top_peak_rss_mb_values, 0.50) if top_peak_rss_mb_values else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(top_peak_rss_mb_values, 0.95) if top_peak_rss_mb_values else math.nan
                    ),
                    "max": maybe_num(max(top_peak_rss_mb_values) if top_peak_rss_mb_values else math.nan),
                },
                "mean_rss_mb": {
                    "mean": maybe_num(
                        statistics.fmean(top_mean_rss_mb_values) if top_mean_rss_mb_values else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(top_mean_rss_mb_values, 0.50) if top_mean_rss_mb_values else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(top_mean_rss_mb_values, 0.95) if top_mean_rss_mb_values else math.nan
                    ),
                },
                "peak_cpu_pct": {
                    "mean": maybe_num(
                        statistics.fmean(top_peak_cpu_pct_values) if top_peak_cpu_pct_values else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(top_peak_cpu_pct_values, 0.50) if top_peak_cpu_pct_values else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(top_peak_cpu_pct_values, 0.95) if top_peak_cpu_pct_values else math.nan
                    ),
                    "max": maybe_num(max(top_peak_cpu_pct_values) if top_peak_cpu_pct_values else math.nan),
                },
                "mean_cpu_pct": {
                    "mean": maybe_num(
                        statistics.fmean(top_mean_cpu_pct_values) if top_mean_cpu_pct_values else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(top_mean_cpu_pct_values, 0.50) if top_mean_cpu_pct_values else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(top_mean_cpu_pct_values, 0.95) if top_mean_cpu_pct_values else math.nan
                    ),
                },
            },
            "vmmap_snapshots": {
                "start_physical_footprint_mb": {
                    "mean": maybe_num(
                        statistics.fmean(vmmap_start_physical_values) if vmmap_start_physical_values else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(vmmap_start_physical_values, 0.50) if vmmap_start_physical_values else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(vmmap_start_physical_values, 0.95) if vmmap_start_physical_values else math.nan
                    ),
                },
                "mid_physical_footprint_mb": {
                    "mean": maybe_num(
                        statistics.fmean(vmmap_mid_physical_values) if vmmap_mid_physical_values else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(vmmap_mid_physical_values, 0.50) if vmmap_mid_physical_values else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(vmmap_mid_physical_values, 0.95) if vmmap_mid_physical_values else math.nan
                    ),
                },
                "end_physical_footprint_mb": {
                    "mean": maybe_num(
                        statistics.fmean(vmmap_end_physical_values) if vmmap_end_physical_values else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(vmmap_end_physical_values, 0.50) if vmmap_end_physical_values else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(vmmap_end_physical_values, 0.95) if vmmap_end_physical_values else math.nan
                    ),
                },
            },
            "xctrace": {
                "trace_count": len(xctrace_traces),
                "trace_paths": xctrace_traces,
                "hotspots_top": hotspot_top,
            },
            "otel_turn_action_stream": {
                "turn_metric_points": {
                    "mean": maybe_num(statistics.fmean(turn_metric_points_values)),
                    "p50": maybe_num(percentile(turn_metric_points_values, 0.50)),
                    "p95": maybe_num(percentile(turn_metric_points_values, 0.95)),
                    "total": int(sum(turn_metric_points_values)),
                },
                "action_metric_points": {
                    "mean": maybe_num(statistics.fmean(action_metric_points_values)),
                    "p50": maybe_num(percentile(action_metric_points_values, 0.50)),
                    "p95": maybe_num(percentile(action_metric_points_values, 0.95)),
                    "total": int(sum(action_metric_points_values)),
                },
                "stream_metric_points": {
                    "mean": maybe_num(statistics.fmean(stream_metric_points_values)),
                    "p50": maybe_num(percentile(stream_metric_points_values, 0.50)),
                    "p95": maybe_num(percentile(stream_metric_points_values, 0.95)),
                    "total": int(sum(stream_metric_points_values)),
                },
                "turn_metric_value_sum": {
                    "mean": maybe_num(
                        statistics.fmean(turn_metric_value_sums) if turn_metric_value_sums else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(turn_metric_value_sums, 0.50) if turn_metric_value_sums else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(turn_metric_value_sums, 0.95) if turn_metric_value_sums else math.nan
                    ),
                },
                "action_metric_value_sum": {
                    "mean": maybe_num(
                        statistics.fmean(action_metric_value_sums) if action_metric_value_sums else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(action_metric_value_sums, 0.50) if action_metric_value_sums else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(action_metric_value_sums, 0.95) if action_metric_value_sums else math.nan
                    ),
                },
                "stream_metric_value_sum": {
                    "mean": maybe_num(
                        statistics.fmean(stream_metric_value_sums) if stream_metric_value_sums else math.nan
                    ),
                    "p50": maybe_num(
                        percentile(stream_metric_value_sums, 0.50) if stream_metric_value_sums else math.nan
                    ),
                    "p95": maybe_num(
                        percentile(stream_metric_value_sums, 0.95) if stream_metric_value_sums else math.nan
                    ),
                },
            },
            "queue_cancel_metrics": queue_cancel_totals,
            "otel_payloads_total": sum(it.otel_payload_count for it in iterations),
            "metric_datapoints_total": sum(it.metric_datapoint_count for it in iterations),
            "successful_runs_total": successful_runs_total,
            "failed_runs_total": failed_runs_total,
            "failed_iterations": [it.iteration for it in iterations if it.return_code != 0],
        },
        "runs": [
            {
                "iteration": it.iteration,
                "duration_ms": round(it.duration_ms, 3),
                "throughput_runs_per_sec": round(it.throughput_runs_per_sec, 6),
                "max_rss_kb": it.max_rss_kb,
                "max_rss_mb": maybe_num((it.max_rss_kb / 1024.0) if it.max_rss_kb is not None else math.nan),
                "user_cpu_sec": it.user_cpu_sec,
                "system_cpu_sec": it.system_cpu_sec,
                "cpu_pct": it.cpu_pct,
                "voluntary_ctx_switches": it.voluntary_ctx_switches,
                "involuntary_ctx_switches": it.involuntary_ctx_switches,
                "peak_open_fds": it.peak_open_fds,
                "peak_direct_children": it.peak_direct_children,
                "return_code": it.return_code,
                "worker_count": it.worker_count,
                "successful_runs": it.successful_runs,
                "failed_runs": it.failed_runs,
                "otel_payload_count": it.otel_payload_count,
                "metric_datapoint_count": it.metric_datapoint_count,
                "queue_cancel_datapoints": it.queue_cancel_datapoints,
                "turn_metric_points": it.turn_metric_points,
                "action_metric_points": it.action_metric_points,
                "stream_metric_points": it.stream_metric_points,
                "turn_metric_value_sum": it.turn_metric_value_sum,
                "action_metric_value_sum": it.action_metric_value_sum,
                "stream_metric_value_sum": it.stream_metric_value_sum,
                "sampled_peak_tree_rss_kb": it.sampled_peak_tree_rss_kb,
                "sampled_peak_tree_rss_mb": maybe_num(
                    (it.sampled_peak_tree_rss_kb / 1024.0)
                    if it.sampled_peak_tree_rss_kb is not None
                    else math.nan
                ),
                "sampled_peak_tree_cpu_pct": it.sampled_peak_tree_cpu_pct,
                "sampled_mean_tree_cpu_pct": it.sampled_mean_tree_cpu_pct,
                "build_cmd_ms": it.build_cmd_ms,
                "spawn_proc_ms": it.spawn_proc_ms,
                "monitor_loop_ms": it.monitor_loop_ms,
                "communicate_ms": it.communicate_ms,
                "parse_stats_ms": it.parse_stats_ms,
                "top_sample_count": it.top_sample_count,
                "top_peak_rss_mb": it.top_peak_rss_mb,
                "top_mean_rss_mb": it.top_mean_rss_mb,
                "top_peak_cpu_pct": it.top_peak_cpu_pct,
                "top_mean_cpu_pct": it.top_mean_cpu_pct,
                "vmmap_start_physical_footprint_mb": it.vmmap_start_physical_footprint_mb,
                "vmmap_mid_physical_footprint_mb": it.vmmap_mid_physical_footprint_mb,
                "vmmap_end_physical_footprint_mb": it.vmmap_end_physical_footprint_mb,
                "xctrace_trace_path": it.xctrace_trace_path,
                "xctrace_hotspots": it.xctrace_hotspots,
                "stderr_tail": it.stderr_tail,
                **(
                    {
                        "worker_results": [
                            {
                                "worker_id": worker.worker_id,
                                "return_code": worker.return_code,
                                "duration_ms": round(worker.duration_ms, 3),
                                "max_rss_kb": worker.max_rss_kb,
                                "max_rss_mb": maybe_num(
                                    (worker.max_rss_kb / 1024.0)
                                    if worker.max_rss_kb is not None
                                    else math.nan
                                ),
                                "user_cpu_sec": worker.user_cpu_sec,
                                "system_cpu_sec": worker.system_cpu_sec,
                                "cpu_pct": worker.cpu_pct,
                                "voluntary_ctx_switches": worker.voluntary_ctx_switches,
                                "involuntary_ctx_switches": worker.involuntary_ctx_switches,
                                "peak_open_fds": worker.peak_open_fds,
                                "peak_direct_children": worker.peak_direct_children,
                                "top_sample_count": worker.top_sample_count,
                                "top_peak_rss_mb": worker.top_peak_rss_mb,
                                "top_mean_rss_mb": worker.top_mean_rss_mb,
                                "top_peak_cpu_pct": worker.top_peak_cpu_pct,
                                "top_mean_cpu_pct": worker.top_mean_cpu_pct,
                                "vmmap_start_physical_footprint_mb": worker.vmmap_start_physical_footprint_mb,
                                "vmmap_mid_physical_footprint_mb": worker.vmmap_mid_physical_footprint_mb,
                                "vmmap_end_physical_footprint_mb": worker.vmmap_end_physical_footprint_mb,
                                "xctrace_trace_path": worker.xctrace_trace_path,
                                "xctrace_hotspots": worker.xctrace_hotspots,
                            }
                            for worker in it.worker_results
                        ]
                    }
                    if it.worker_results
                    else {}
                ),
            }
            for it in iterations
        ],
    }


def write_markdown(summary: dict[str, Any], out_path: Path) -> None:
    stats = summary["summary"]
    profile = summary.get("profile", {})

    def fmt(value: Any, digits: int = 3) -> str:
        if value is None:
            return "n/a"
        if isinstance(value, (int, float)):
            return f"{value:.{digits}f}"
        return str(value)

    lines = [
        "# Codex Local Perf Summary",
        "",
        f"- Generated: `{summary['generated_at']}`",
        f"- Command: `{summary['command']}`",
        f"- Iterations: `{summary['iterations']}`",
        f"- Config: `{summary['config_path']}`",
        "",
        "## Profile",
        "",
        f"- Name: `{profile.get('name')}`",
        f"- Phase: `{profile.get('phase')}`",
        f"- Concurrency: `{profile.get('concurrency')}`",
        f"- Warmup: `{profile.get('warmup')}`",
        f"- Measured iterations: `{profile.get('iterations')}`",
        "",
        "## Totals",
        "",
        f"- Successful runs: `{stats.get('successful_runs_total', 0)}`",
        f"- Failed runs: `{stats.get('failed_runs_total', 0)}`",
        "",
        "## Latency / Throughput / RSS",
        "",
        "| Metric | Mean | P50 | P95 | Min | Max |",
        "|---|---:|---:|---:|---:|---:|",
        (
            "| latency_ms | "
            f"{fmt(stats['latency_ms']['mean'])} | {fmt(stats['latency_ms']['p50'])} | "
            f"{fmt(stats['latency_ms']['p95'])} | {fmt(stats['latency_ms']['min'])} | "
            f"{fmt(stats['latency_ms']['max'])} |"
        ),
        (
            "| throughput_runs_per_sec | "
            f"{fmt(stats['throughput_runs_per_sec']['mean'], 4)} | "
            f"{fmt(stats['throughput_runs_per_sec']['p50'], 4)} | "
            f"{fmt(stats['throughput_runs_per_sec']['p95'], 4)} | n/a | n/a |"
        ),
        (
            "| max_rss_mb | "
            f"{fmt(stats['max_rss_mb']['mean'], 2)} | {fmt(stats['max_rss_mb']['p50'], 2)} | "
            f"{fmt(stats['max_rss_mb']['p95'], 2)} | {fmt(stats['max_rss_mb']['min'], 2)} | "
            f"{fmt(stats['max_rss_mb']['max'], 2)} |"
        ),
        "",
        "## CPU / Scheduler / Process Shape",
        "",
        "| Metric | Mean | P50 | P95 | Min | Max |",
        "|---|---:|---:|---:|---:|---:|",
        (
            "| user_cpu_sec | "
            f"{fmt(stats['user_cpu_sec']['mean'])} | {fmt(stats['user_cpu_sec']['p50'])} | "
            f"{fmt(stats['user_cpu_sec']['p95'])} | n/a | n/a |"
        ),
        (
            "| system_cpu_sec | "
            f"{fmt(stats['system_cpu_sec']['mean'])} | {fmt(stats['system_cpu_sec']['p50'])} | "
            f"{fmt(stats['system_cpu_sec']['p95'])} | n/a | n/a |"
        ),
        (
            "| cpu_pct | "
            f"{fmt(stats['cpu_pct']['mean'])} | {fmt(stats['cpu_pct']['p50'])} | "
            f"{fmt(stats['cpu_pct']['p95'])} | n/a | n/a |"
        ),
        (
            "| voluntary_ctx_switches | "
            f"{fmt(stats['voluntary_ctx_switches']['mean'], 1)} | "
            f"{fmt(stats['voluntary_ctx_switches']['p50'], 1)} | "
            f"{fmt(stats['voluntary_ctx_switches']['p95'], 1)} | n/a | n/a |"
        ),
        (
            "| involuntary_ctx_switches | "
            f"{fmt(stats['involuntary_ctx_switches']['mean'], 1)} | "
            f"{fmt(stats['involuntary_ctx_switches']['p50'], 1)} | "
            f"{fmt(stats['involuntary_ctx_switches']['p95'], 1)} | n/a | n/a |"
        ),
        (
            "| peak_open_fds | "
            f"{fmt(stats['peak_open_fds']['mean'], 1)} | {fmt(stats['peak_open_fds']['p50'], 1)} | "
            f"{fmt(stats['peak_open_fds']['p95'], 1)} | n/a | {fmt(stats['peak_open_fds']['max'], 1)} |"
        ),
        (
            "| peak_direct_children | "
            f"{fmt(stats['peak_direct_children']['mean'], 1)} | "
            f"{fmt(stats['peak_direct_children']['p50'], 1)} | "
            f"{fmt(stats['peak_direct_children']['p95'], 1)} | n/a | "
            f"{fmt(stats['peak_direct_children']['max'], 1)} |"
        ),
        "",
        "## Process Tree Samples",
        "",
        "| Metric | Mean | P50 | P95 | Max |",
        "|---|---:|---:|---:|---:|",
        (
            "| sampled_peak_tree_rss_mb | "
            f"{fmt(stats.get('process_tree_sampled', {}).get('peak_tree_rss_mb', {}).get('mean'), 2)} | "
            f"{fmt(stats.get('process_tree_sampled', {}).get('peak_tree_rss_mb', {}).get('p50'), 2)} | "
            f"{fmt(stats.get('process_tree_sampled', {}).get('peak_tree_rss_mb', {}).get('p95'), 2)} | "
            f"{fmt(stats.get('process_tree_sampled', {}).get('peak_tree_rss_mb', {}).get('max'), 2)} |"
        ),
        (
            "| sampled_peak_tree_cpu_pct | "
            f"{fmt(stats.get('process_tree_sampled', {}).get('peak_tree_cpu_pct', {}).get('mean'), 2)} | "
            f"{fmt(stats.get('process_tree_sampled', {}).get('peak_tree_cpu_pct', {}).get('p50'), 2)} | "
            f"{fmt(stats.get('process_tree_sampled', {}).get('peak_tree_cpu_pct', {}).get('p95'), 2)} | "
            f"{fmt(stats.get('process_tree_sampled', {}).get('peak_tree_cpu_pct', {}).get('max'), 2)} |"
        ),
        (
            "| sampled_mean_tree_cpu_pct | "
            f"{fmt(stats.get('process_tree_sampled', {}).get('mean_tree_cpu_pct', {}).get('mean'), 2)} | "
            f"{fmt(stats.get('process_tree_sampled', {}).get('mean_tree_cpu_pct', {}).get('p50'), 2)} | "
            f"{fmt(stats.get('process_tree_sampled', {}).get('mean_tree_cpu_pct', {}).get('p95'), 2)} | n/a |"
        ),
        "",
        "## Worker Step Timings (ms)",
        "",
        "| Step | Mean | P50 | P95 |",
        "|---|---:|---:|---:|",
        (
            "| build_cmd | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('build_cmd', {}).get('mean'), 3)} | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('build_cmd', {}).get('p50'), 3)} | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('build_cmd', {}).get('p95'), 3)} |"
        ),
        (
            "| spawn_proc | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('spawn_proc', {}).get('mean'), 3)} | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('spawn_proc', {}).get('p50'), 3)} | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('spawn_proc', {}).get('p95'), 3)} |"
        ),
        (
            "| monitor_loop | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('monitor_loop', {}).get('mean'), 3)} | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('monitor_loop', {}).get('p50'), 3)} | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('monitor_loop', {}).get('p95'), 3)} |"
        ),
        (
            "| communicate | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('communicate', {}).get('mean'), 3)} | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('communicate', {}).get('p50'), 3)} | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('communicate', {}).get('p95'), 3)} |"
        ),
        (
            "| parse_stats | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('parse_stats', {}).get('mean'), 3)} | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('parse_stats', {}).get('p50'), 3)} | "
            f"{fmt(stats.get('worker_step_timings_ms', {}).get('parse_stats', {}).get('p95'), 3)} |"
        ),
        "",
        "## Resource Budget (Normalized)",
        "",
        "| Budget | Metric | Value |",
        "|---|---|---:|",
        (
            "| time | build_cmd_share_pct | "
            f"{fmt(stats.get('resource_budget', {}).get('time_budget_ms', {}).get('build_cmd_share_pct'), 2)} |"
        ),
        (
            "| time | spawn_proc_share_pct | "
            f"{fmt(stats.get('resource_budget', {}).get('time_budget_ms', {}).get('spawn_proc_share_pct'), 2)} |"
        ),
        (
            "| time | monitor_loop_share_pct | "
            f"{fmt(stats.get('resource_budget', {}).get('time_budget_ms', {}).get('monitor_loop_share_pct'), 2)} |"
        ),
        (
            "| time | communicate_share_pct | "
            f"{fmt(stats.get('resource_budget', {}).get('time_budget_ms', {}).get('communicate_share_pct'), 2)} |"
        ),
        (
            "| time | parse_stats_share_pct | "
            f"{fmt(stats.get('resource_budget', {}).get('time_budget_ms', {}).get('parse_stats_share_pct'), 2)} |"
        ),
        (
            "| time | unaccounted_share_pct | "
            f"{fmt(stats.get('resource_budget', {}).get('time_budget_ms', {}).get('unaccounted_share_pct'), 2)} |"
        ),
        (
            "| cpu | cpu_core_utilization_pct | "
            f"{fmt(stats.get('resource_budget', {}).get('cpu_budget', {}).get('cpu_core_utilization_pct'), 2)} |"
        ),
        (
            "| process | peak_open_fds_mean | "
            f"{fmt(stats.get('resource_budget', {}).get('process_budget', {}).get('peak_open_fds_mean'), 2)} |"
        ),
        (
            "| process | peak_direct_children_mean | "
            f"{fmt(stats.get('resource_budget', {}).get('process_budget', {}).get('peak_direct_children_mean'), 2)} |"
        ),
        (
            "| process | sampled_peak_tree_rss_mean_mb | "
            f"{fmt(stats.get('resource_budget', {}).get('process_budget', {}).get('sampled_peak_tree_rss_mean_mb'), 2)} |"
        ),
        (
            "| process | sampled_peak_tree_cpu_p95 | "
            f"{fmt(stats.get('resource_budget', {}).get('process_budget', {}).get('sampled_peak_tree_cpu_p95'), 2)} |"
        ),
        (
            "| stability | failure_rate_pct | "
            f"{fmt(stats.get('resource_budget', {}).get('stability', {}).get('failure_rate_pct'), 2)} |"
        ),
        (
            "| stability | timeout_rate_pct | "
            f"{fmt(stats.get('resource_budget', {}).get('stability', {}).get('timeout_rate_pct'), 2)} |"
        ),
        "",
        "## top Attach Samples",
        "",
        "| Metric | Mean | P50 | P95 | Max |",
        "|---|---:|---:|---:|---:|",
        (
            "| top_sample_count | "
            f"{fmt(stats.get('top_attach', {}).get('sample_count', {}).get('mean'), 1)} | "
            f"{fmt(stats.get('top_attach', {}).get('sample_count', {}).get('p50'), 1)} | "
            f"{fmt(stats.get('top_attach', {}).get('sample_count', {}).get('p95'), 1)} | n/a |"
        ),
        (
            "| top_peak_rss_mb | "
            f"{fmt(stats.get('top_attach', {}).get('peak_rss_mb', {}).get('mean'), 2)} | "
            f"{fmt(stats.get('top_attach', {}).get('peak_rss_mb', {}).get('p50'), 2)} | "
            f"{fmt(stats.get('top_attach', {}).get('peak_rss_mb', {}).get('p95'), 2)} | "
            f"{fmt(stats.get('top_attach', {}).get('peak_rss_mb', {}).get('max'), 2)} |"
        ),
        (
            "| top_mean_rss_mb | "
            f"{fmt(stats.get('top_attach', {}).get('mean_rss_mb', {}).get('mean'), 2)} | "
            f"{fmt(stats.get('top_attach', {}).get('mean_rss_mb', {}).get('p50'), 2)} | "
            f"{fmt(stats.get('top_attach', {}).get('mean_rss_mb', {}).get('p95'), 2)} | n/a |"
        ),
        (
            "| top_peak_cpu_pct | "
            f"{fmt(stats.get('top_attach', {}).get('peak_cpu_pct', {}).get('mean'), 2)} | "
            f"{fmt(stats.get('top_attach', {}).get('peak_cpu_pct', {}).get('p50'), 2)} | "
            f"{fmt(stats.get('top_attach', {}).get('peak_cpu_pct', {}).get('p95'), 2)} | "
            f"{fmt(stats.get('top_attach', {}).get('peak_cpu_pct', {}).get('max'), 2)} |"
        ),
        (
            "| top_mean_cpu_pct | "
            f"{fmt(stats.get('top_attach', {}).get('mean_cpu_pct', {}).get('mean'), 2)} | "
            f"{fmt(stats.get('top_attach', {}).get('mean_cpu_pct', {}).get('p50'), 2)} | "
            f"{fmt(stats.get('top_attach', {}).get('mean_cpu_pct', {}).get('p95'), 2)} | n/a |"
        ),
        "",
        "## vmmap Snapshots",
        "",
        "| Metric | Mean | P50 | P95 |",
        "|---|---:|---:|---:|",
        (
            "| vmmap_start_physical_footprint_mb | "
            f"{fmt(stats.get('vmmap_snapshots', {}).get('start_physical_footprint_mb', {}).get('mean'), 2)} | "
            f"{fmt(stats.get('vmmap_snapshots', {}).get('start_physical_footprint_mb', {}).get('p50'), 2)} | "
            f"{fmt(stats.get('vmmap_snapshots', {}).get('start_physical_footprint_mb', {}).get('p95'), 2)} |"
        ),
        (
            "| vmmap_mid_physical_footprint_mb | "
            f"{fmt(stats.get('vmmap_snapshots', {}).get('mid_physical_footprint_mb', {}).get('mean'), 2)} | "
            f"{fmt(stats.get('vmmap_snapshots', {}).get('mid_physical_footprint_mb', {}).get('p50'), 2)} | "
            f"{fmt(stats.get('vmmap_snapshots', {}).get('mid_physical_footprint_mb', {}).get('p95'), 2)} |"
        ),
        (
            "| vmmap_end_physical_footprint_mb | "
            f"{fmt(stats.get('vmmap_snapshots', {}).get('end_physical_footprint_mb', {}).get('mean'), 2)} | "
            f"{fmt(stats.get('vmmap_snapshots', {}).get('end_physical_footprint_mb', {}).get('p50'), 2)} | "
            f"{fmt(stats.get('vmmap_snapshots', {}).get('end_physical_footprint_mb', {}).get('p95'), 2)} |"
        ),
        "",
        "## xctrace Hotspots",
        "",
    ]

    xctrace_stats = stats.get("xctrace", {})
    trace_count = xctrace_stats.get("trace_count", 0)
    lines.append(f"- Trace captures: `{trace_count}`")
    hotspots = xctrace_stats.get("hotspots_top", [])
    if hotspots:
        lines.append("")
        lines.append("| Frame | Weight (ms) | Samples |")
        lines.append("|---|---:|---:|")
        for item in hotspots:
            lines.append(
                f"| `{item.get('frame', '<unknown>')}` | "
                f"{fmt(item.get('weight_ms'), 3)} | {int(item.get('samples', 0))} |"
            )
    else:
        lines.append("- No hotspots captured.")
    lines.extend(["", "## Queue / Cancel Metrics", ""])

    queue_cancel = stats["queue_cancel_metrics"]
    if queue_cancel:
        lines.append("| Metric name | Datapoints |")
        lines.append("|---|---:|")
        for name, count in sorted(queue_cancel.items()):
            lines.append(f"| `{name}` | {count} |")
    else:
        lines.append("No queue/cancel metric datapoints were observed.")

    tas = stats.get("otel_turn_action_stream", {})
    if tas:
        lines.extend(
            [
                "",
                "## Turn / Action / Streaming OTEL Signals",
                "",
                "| Signal | Mean points | P50 points | P95 points | Total points | Mean value-sum |",
                "|---|---:|---:|---:|---:|---:|",
                (
                    "| turn | "
                    f"{fmt(tas.get('turn_metric_points', {}).get('mean'), 2)} | "
                    f"{fmt(tas.get('turn_metric_points', {}).get('p50'), 2)} | "
                    f"{fmt(tas.get('turn_metric_points', {}).get('p95'), 2)} | "
                    f"{tas.get('turn_metric_points', {}).get('total', 0)} | "
                    f"{fmt(tas.get('turn_metric_value_sum', {}).get('mean'), 3)} |"
                ),
                (
                    "| action | "
                    f"{fmt(tas.get('action_metric_points', {}).get('mean'), 2)} | "
                    f"{fmt(tas.get('action_metric_points', {}).get('p50'), 2)} | "
                    f"{fmt(tas.get('action_metric_points', {}).get('p95'), 2)} | "
                    f"{tas.get('action_metric_points', {}).get('total', 0)} | "
                    f"{fmt(tas.get('action_metric_value_sum', {}).get('mean'), 3)} |"
                ),
                (
                    "| stream | "
                    f"{fmt(tas.get('stream_metric_points', {}).get('mean'), 2)} | "
                    f"{fmt(tas.get('stream_metric_points', {}).get('p50'), 2)} | "
                    f"{fmt(tas.get('stream_metric_points', {}).get('p95'), 2)} | "
                    f"{tas.get('stream_metric_points', {}).get('total', 0)} | "
                    f"{fmt(tas.get('stream_metric_value_sum', {}).get('mean'), 3)} |"
                ),
            ]
        )

    lines.extend(
        [
            "",
            "## Iteration Return Codes",
            "",
            "| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (MB) | OTEL payloads |",
            "|---|---:|---:|---:|---:|---:|---:|---:|",
        ]
    )

    for run in summary["runs"]:
        rss = "n/a" if run.get("max_rss_mb") is None else f"{run['max_rss_mb']:.2f}"
        lines.append(
            f"| {run['iteration']} | {run['return_code']} | {run['worker_count']} | "
            f"{run['successful_runs']} | {run['failed_runs']} | {run['duration_ms']:.3f} | {rss} | "
            f"{run['otel_payload_count']} |"
        )

    out_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def run_iteration(
    iteration: int,
    cmd: str,
    worker_envs: list[dict[str, str]],
    timeout_sec: float,
    enable_top_attach: bool,
    top_interval_sec: float,
    enable_vmmap_snapshots: bool,
    enable_xctrace_capture: bool,
    xctrace_time_limit_sec: float,
    xctrace_hotspots_limit: int,
    xctrace_artifact_dir: Path | None,
    monitor_sleep_sec: float,
    probe_interval_sec: float,
    otel_flush_wait_sec: float,
    collector_state: _CollectorState,
) -> IterationResult:
    collector_state.clear()

    start = time.perf_counter()
    worker_results: list[WorkerResult]

    if len(worker_envs) == 1:
        worker_results = [
            _run_worker(
                1,
                cmd,
                worker_envs[0],
                timeout_sec,
                enable_top_attach,
                top_interval_sec,
                enable_vmmap_snapshots,
                enable_xctrace_capture,
                xctrace_time_limit_sec,
                xctrace_hotspots_limit,
                xctrace_artifact_dir,
                monitor_sleep_sec,
                probe_interval_sec,
            )
        ]
    else:
        with ThreadPoolExecutor(max_workers=len(worker_envs)) as executor:
            futures = [
                executor.submit(
                    _run_worker,
                    idx + 1,
                    cmd,
                    worker_envs[idx],
                    timeout_sec,
                    enable_top_attach,
                    top_interval_sec,
                    enable_vmmap_snapshots,
                    enable_xctrace_capture,
                    xctrace_time_limit_sec,
                    xctrace_hotspots_limit,
                    xctrace_artifact_dir,
                    monitor_sleep_sec,
                    probe_interval_sec,
                )
                for idx in range(len(worker_envs))
            ]
            worker_results = [future.result() for future in futures]

    end = time.perf_counter()

    # Allow async metric flush to reach local collector.
    time.sleep(max(otel_flush_wait_sec, 0.0))

    payloads = collector_state.snapshot()
    points: list[dict[str, Any]] = []
    for record in payloads:
        points.extend(metric_points(record.get("body")))

    queue_cancel: dict[str, int] = {}
    for point in points:
        name = point.get("name", "")
        if isinstance(name, str) and QUEUE_CANCEL_RE.search(name.lower()):
            queue_cancel[name] = queue_cancel.get(name, 0) + int(point.get("count", 1))

    def category_stats(pattern: re.Pattern[str]) -> tuple[int, float | None]:
        matched = [
            point
            for point in points
            if isinstance(point.get("name"), str) and pattern.search(point["name"])
        ]
        values = [point.get("value") for point in matched if isinstance(point.get("value"), (int, float))]
        return len(matched), (float(sum(values)) if values else None)

    turn_metric_points, turn_metric_value_sum = category_stats(TURN_LATENCY_RE)
    action_metric_points, action_metric_value_sum = category_stats(ACTION_LATENCY_RE)
    stream_metric_points, stream_metric_value_sum = category_stats(STREAM_LATENCY_RE)

    duration_ms = (end - start) * 1000.0
    successful_runs = sum(1 for worker in worker_results if worker.return_code == 0)
    failed_runs = len(worker_results) - successful_runs
    throughput = successful_runs / (duration_ms / 1000.0) if duration_ms > 0 else 0.0

    rss_candidates = [worker.max_rss_kb for worker in worker_results if worker.max_rss_kb is not None]
    max_rss_kb = max(rss_candidates) if rss_candidates else None

    return_code = 0 if failed_runs == 0 else 1
    if len(worker_results) == 1:
        stderr_tail = worker_results[0].stderr_tail
    else:
        stderr_lines = [
            f"worker={worker.worker_id} rc={worker.return_code}: {worker.stderr_tail}"
            for worker in worker_results
            if worker.return_code != 0 and worker.stderr_tail
        ]
        stderr_tail = "\n".join(stderr_lines[-5:])

    return IterationResult(
            iteration=iteration,
            duration_ms=duration_ms,
            throughput_runs_per_sec=throughput,
            max_rss_kb=max_rss_kb,
            user_cpu_sec=statistics.fmean(
                [worker.user_cpu_sec for worker in worker_results if worker.user_cpu_sec is not None]
            )
            if any(worker.user_cpu_sec is not None for worker in worker_results)
            else None,
            system_cpu_sec=statistics.fmean(
                [worker.system_cpu_sec for worker in worker_results if worker.system_cpu_sec is not None]
            )
            if any(worker.system_cpu_sec is not None for worker in worker_results)
            else None,
            cpu_pct=statistics.fmean(
                [worker.cpu_pct for worker in worker_results if worker.cpu_pct is not None]
            )
            if any(worker.cpu_pct is not None for worker in worker_results)
            else None,
            voluntary_ctx_switches=max(
                [worker.voluntary_ctx_switches for worker in worker_results if worker.voluntary_ctx_switches is not None],
                default=None,
            ),
            involuntary_ctx_switches=max(
                [
                    worker.involuntary_ctx_switches
                    for worker in worker_results
                    if worker.involuntary_ctx_switches is not None
                ],
                default=None,
            ),
            peak_open_fds=max(
                [worker.peak_open_fds for worker in worker_results if worker.peak_open_fds is not None],
                default=None,
            ),
            peak_direct_children=max(
                [
                    worker.peak_direct_children
                    for worker in worker_results
                    if worker.peak_direct_children is not None
                ],
                default=None,
            ),
            sampled_peak_tree_rss_kb=max(
                [
                    worker.sampled_peak_tree_rss_kb
                    for worker in worker_results
                    if worker.sampled_peak_tree_rss_kb is not None
                ],
                default=None,
            ),
            sampled_peak_tree_cpu_pct=max(
                [
                    worker.sampled_peak_tree_cpu_pct
                    for worker in worker_results
                    if worker.sampled_peak_tree_cpu_pct is not None
                ],
                default=None,
            ),
            sampled_mean_tree_cpu_pct=statistics.fmean(
                [
                    worker.sampled_mean_tree_cpu_pct
                    for worker in worker_results
                    if worker.sampled_mean_tree_cpu_pct is not None
                ]
            )
            if any(worker.sampled_mean_tree_cpu_pct is not None for worker in worker_results)
            else None,
            build_cmd_ms=statistics.fmean([worker.build_cmd_ms for worker in worker_results]),
            spawn_proc_ms=statistics.fmean([worker.spawn_proc_ms for worker in worker_results]),
            monitor_loop_ms=statistics.fmean([worker.monitor_loop_ms for worker in worker_results]),
            communicate_ms=statistics.fmean([worker.communicate_ms for worker in worker_results]),
            parse_stats_ms=statistics.fmean([worker.parse_stats_ms for worker in worker_results]),
            top_sample_count=max((worker.top_sample_count for worker in worker_results), default=0),
            top_peak_rss_mb=max(
                [worker.top_peak_rss_mb for worker in worker_results if worker.top_peak_rss_mb is not None],
                default=None,
            ),
            top_mean_rss_mb=statistics.fmean(
                [worker.top_mean_rss_mb for worker in worker_results if worker.top_mean_rss_mb is not None]
            )
            if any(worker.top_mean_rss_mb is not None for worker in worker_results)
            else None,
            top_peak_cpu_pct=max(
                [worker.top_peak_cpu_pct for worker in worker_results if worker.top_peak_cpu_pct is not None],
                default=None,
            ),
            top_mean_cpu_pct=statistics.fmean(
                [worker.top_mean_cpu_pct for worker in worker_results if worker.top_mean_cpu_pct is not None]
            )
            if any(worker.top_mean_cpu_pct is not None for worker in worker_results)
            else None,
            vmmap_start_physical_footprint_mb=statistics.fmean(
                [
                    worker.vmmap_start_physical_footprint_mb
                    for worker in worker_results
                    if worker.vmmap_start_physical_footprint_mb is not None
                ]
            )
            if any(worker.vmmap_start_physical_footprint_mb is not None for worker in worker_results)
            else None,
            vmmap_mid_physical_footprint_mb=statistics.fmean(
                [
                    worker.vmmap_mid_physical_footprint_mb
                    for worker in worker_results
                    if worker.vmmap_mid_physical_footprint_mb is not None
                ]
            )
            if any(worker.vmmap_mid_physical_footprint_mb is not None for worker in worker_results)
            else None,
            vmmap_end_physical_footprint_mb=statistics.fmean(
                [
                    worker.vmmap_end_physical_footprint_mb
                    for worker in worker_results
                    if worker.vmmap_end_physical_footprint_mb is not None
                ]
            )
            if any(worker.vmmap_end_physical_footprint_mb is not None for worker in worker_results)
            else None,
            xctrace_trace_path=next(
                (worker.xctrace_trace_path for worker in worker_results if worker.xctrace_trace_path),
                None,
            ),
            xctrace_hotspots=next(
                (worker.xctrace_hotspots for worker in worker_results if worker.xctrace_hotspots),
                None,
            ),
            return_code=return_code,
            worker_count=len(worker_results),
            successful_runs=successful_runs,
        failed_runs=failed_runs,
        otel_payload_count=len(payloads),
        metric_datapoint_count=len(points),
        queue_cancel_datapoints=queue_cancel,
        turn_metric_points=turn_metric_points,
        action_metric_points=action_metric_points,
        stream_metric_points=stream_metric_points,
        turn_metric_value_sum=turn_metric_value_sum,
        action_metric_value_sum=action_metric_value_sum,
        stream_metric_value_sum=stream_metric_value_sum,
        stderr_tail=stderr_tail,
        worker_results=worker_results if len(worker_results) > 1 else None,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Local perf benchmark using codex-otel hooks.")
    parser.add_argument("--cmd", required=True, help="Command to benchmark (quoted).")
    parser.add_argument("--iterations", type=int, default=5, help="Measured iterations.")
    parser.add_argument("--warmup", type=int, default=1, help="Warmup runs before measuring.")
    parser.add_argument(
        "--concurrency",
        type=int,
        default=1,
        help="Number of parallel command invocations per measured iteration.",
    )
    parser.add_argument(
        "--profile-name",
        default=None,
        help="Optional profile name metadata for the benchmark run.",
    )
    parser.add_argument(
        "--profile-phase",
        default="measure",
        help="Profile phase metadata (for example measure, stress, smoke).",
    )
    parser.add_argument(
        "--timeout-sec",
        type=float,
        default=300.0,
        help="Per-worker command timeout in seconds.",
    )
    parser.add_argument(
        "--out-dir",
        default="codex-rs/perf-results",
        help="Directory for JSON/Markdown outputs.",
    )
    parser.add_argument(
        "--name",
        default="local-perf",
        help="Run name prefix for output folder.",
    )
    parser.add_argument(
        "--keep-temp",
        action="store_true",
        help="Keep temporary CODEX_HOME and raw capture artifacts.",
    )
    parser.add_argument(
        "--auth-codex-home",
        default=None,
        help=(
            "Optional existing CODEX_HOME to copy auth.json from into each temporary worker home "
            "(for account-auth runs without OPENAI_API_KEY)."
        ),
    )
    parser.add_argument(
        "--top-attach",
        action="store_true",
        help="Enable macOS top(1) attach sampling for each worker process.",
    )
    parser.add_argument(
        "--top-interval-ms",
        type=float,
        default=250.0,
        help="Sampling interval for top attach mode (milliseconds).",
    )
    parser.add_argument(
        "--vmmap-snapshots",
        action="store_true",
        help="Capture vmmap -summary snapshots (start/mid/end) for each worker (macOS).",
    )
    parser.add_argument(
        "--xctrace-capture",
        action="store_true",
        help="Capture xctrace Time Profiler traces and extract hotspots (macOS).",
    )
    parser.add_argument(
        "--xctrace-time-limit-sec",
        type=float,
        default=8.0,
        help="Time Profiler capture duration in seconds when --xctrace-capture is enabled.",
    )
    parser.add_argument(
        "--xctrace-hotspots-limit",
        type=int,
        default=10,
        help="Number of top hotspot frames to keep per captured trace.",
    )
    parser.add_argument(
        "--monitor-sleep-ms",
        type=float,
        default=120.0,
        help="Sleep duration for worker monitor loop iterations (milliseconds).",
    )
    parser.add_argument(
        "--probe-interval-ms",
        type=float,
        default=120.0,
        help="Interval for expensive process probes (ps/pgrep/vmmap) in monitor loop (milliseconds).",
    )
    parser.add_argument(
        "--otel-flush-wait-ms",
        type=float,
        default=50.0,
        help="Post-run OTEL collector flush wait before payload snapshot (milliseconds).",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.iterations <= 0:
        raise SystemExit("--iterations must be > 0")
    if args.warmup < 0:
        raise SystemExit("--warmup must be >= 0")
    if args.concurrency <= 0:
        raise SystemExit("--concurrency must be > 0")
    if args.top_interval_ms <= 0:
        raise SystemExit("--top-interval-ms must be > 0")
    if args.xctrace_time_limit_sec <= 0:
        raise SystemExit("--xctrace-time-limit-sec must be > 0")
    if args.xctrace_hotspots_limit <= 0:
        raise SystemExit("--xctrace-hotspots-limit must be > 0")
    if args.monitor_sleep_ms <= 0:
        raise SystemExit("--monitor-sleep-ms must be > 0")
    if args.probe_interval_ms <= 0:
        raise SystemExit("--probe-interval-ms must be > 0")
    if args.otel_flush_wait_ms < 0:
        raise SystemExit("--otel-flush-wait-ms must be >= 0")

    run_stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    run_dir = Path(args.out_dir) / f"{args.name}-{run_stamp}"
    run_dir.mkdir(parents=True, exist_ok=True)

    server, collector_state, _thread = start_collector()
    endpoint = f"http://127.0.0.1:{server.server_port}"

    temp_root = Path(tempfile.mkdtemp(prefix="codex-perf-"))
    codex_home_root = temp_root / "codex-home"

    if args.concurrency == 1:
        config_path = write_config(codex_home_root, endpoint)
        worker_homes = [codex_home_root]
    else:
        worker_homes = []
        for worker_id in range(1, args.concurrency + 1):
            worker_home = codex_home_root.parent / f"{codex_home_root.name}-worker-{worker_id}"
            write_config(worker_home, endpoint)
            worker_homes.append(worker_home)
        config_path = codex_home_root.parent / f"{codex_home_root.name}-worker-1" / "config.toml"

    auth_source_home = resolve_codex_home(args.auth_codex_home)
    copied_auth = 0
    for worker_home in worker_homes:
        if copy_account_auth(auth_source_home, worker_home):
            copied_auth += 1

    env = os.environ.copy()
    env.pop("CODEX_HOME", None)
    env.setdefault("RUST_BACKTRACE", "0")
    worker_envs = _build_worker_envs(env, codex_home_root, args.concurrency)

    profile = {
        "name": args.profile_name,
        "phase": args.profile_phase,
        "concurrency": args.concurrency,
        "warmup": args.warmup,
        "iterations": args.iterations,
        "auth_source_home": str(auth_source_home),
        "auth_copied_workers": copied_auth,
        "top_attach": bool(args.top_attach),
        "top_interval_ms": float(args.top_interval_ms),
        "vmmap_snapshots": bool(args.vmmap_snapshots),
        "xctrace_capture": bool(args.xctrace_capture),
        "xctrace_time_limit_sec": float(args.xctrace_time_limit_sec),
        "xctrace_hotspots_limit": int(args.xctrace_hotspots_limit),
        "monitor_sleep_ms": float(args.monitor_sleep_ms),
        "probe_interval_ms": float(args.probe_interval_ms),
        "otel_flush_wait_ms": float(args.otel_flush_wait_ms),
    }
    xctrace_artifact_dir = run_dir / "xctrace-traces" if args.xctrace_capture else None

    all_results: list[IterationResult] = []

    try:
        for i in range(1, args.warmup + 1):
            _ = run_iteration(
                i,
                args.cmd,
                worker_envs,
                args.timeout_sec,
                args.top_attach,
                args.top_interval_ms / 1000.0,
                args.vmmap_snapshots,
                args.xctrace_capture,
                args.xctrace_time_limit_sec,
                args.xctrace_hotspots_limit,
                xctrace_artifact_dir,
                args.monitor_sleep_ms / 1000.0,
                args.probe_interval_ms / 1000.0,
                args.otel_flush_wait_ms / 1000.0,
                collector_state,
            )

        for i in range(1, args.iterations + 1):
            result = run_iteration(
                i,
                args.cmd,
                worker_envs,
                args.timeout_sec,
                args.top_attach,
                args.top_interval_ms / 1000.0,
                args.vmmap_snapshots,
                args.xctrace_capture,
                args.xctrace_time_limit_sec,
                args.xctrace_hotspots_limit,
                xctrace_artifact_dir,
                args.monitor_sleep_ms / 1000.0,
                args.probe_interval_ms / 1000.0,
                args.otel_flush_wait_ms / 1000.0,
                collector_state,
            )
            all_results.append(result)

            iteration_payload: dict[str, Any] = {
                "iteration": result.iteration,
                "duration_ms": result.duration_ms,
                "throughput_runs_per_sec": result.throughput_runs_per_sec,
                "max_rss_kb": result.max_rss_kb,
                "max_rss_mb": (
                    (result.max_rss_kb / 1024.0) if result.max_rss_kb is not None else None
                ),
                "user_cpu_sec": result.user_cpu_sec,
                "system_cpu_sec": result.system_cpu_sec,
                "cpu_pct": result.cpu_pct,
                "voluntary_ctx_switches": result.voluntary_ctx_switches,
                "involuntary_ctx_switches": result.involuntary_ctx_switches,
                "peak_open_fds": result.peak_open_fds,
                "peak_direct_children": result.peak_direct_children,
                "return_code": result.return_code,
                "worker_count": result.worker_count,
                "successful_runs": result.successful_runs,
                "failed_runs": result.failed_runs,
                "otel_payload_count": result.otel_payload_count,
                "metric_datapoint_count": result.metric_datapoint_count,
                "queue_cancel_datapoints": result.queue_cancel_datapoints,
                "turn_metric_points": result.turn_metric_points,
                "action_metric_points": result.action_metric_points,
                "stream_metric_points": result.stream_metric_points,
                "turn_metric_value_sum": result.turn_metric_value_sum,
                "action_metric_value_sum": result.action_metric_value_sum,
                "stream_metric_value_sum": result.stream_metric_value_sum,
                "sampled_peak_tree_rss_kb": result.sampled_peak_tree_rss_kb,
                "sampled_peak_tree_rss_mb": (
                    (result.sampled_peak_tree_rss_kb / 1024.0)
                    if result.sampled_peak_tree_rss_kb is not None
                    else None
                ),
                "sampled_peak_tree_cpu_pct": result.sampled_peak_tree_cpu_pct,
                "sampled_mean_tree_cpu_pct": result.sampled_mean_tree_cpu_pct,
                "build_cmd_ms": result.build_cmd_ms,
                "spawn_proc_ms": result.spawn_proc_ms,
                "monitor_loop_ms": result.monitor_loop_ms,
                "communicate_ms": result.communicate_ms,
                "parse_stats_ms": result.parse_stats_ms,
                "top_sample_count": result.top_sample_count,
                "top_peak_rss_mb": result.top_peak_rss_mb,
                "top_mean_rss_mb": result.top_mean_rss_mb,
                "top_peak_cpu_pct": result.top_peak_cpu_pct,
                "top_mean_cpu_pct": result.top_mean_cpu_pct,
                "vmmap_start_physical_footprint_mb": result.vmmap_start_physical_footprint_mb,
                "vmmap_mid_physical_footprint_mb": result.vmmap_mid_physical_footprint_mb,
                "vmmap_end_physical_footprint_mb": result.vmmap_end_physical_footprint_mb,
                "xctrace_trace_path": result.xctrace_trace_path,
                "xctrace_hotspots": result.xctrace_hotspots,
                "stderr_tail": result.stderr_tail,
            }
            if result.worker_results:
                iteration_payload["worker_results"] = [
                    {
                        "worker_id": worker.worker_id,
                        "return_code": worker.return_code,
                        "duration_ms": worker.duration_ms,
                        "max_rss_kb": worker.max_rss_kb,
                        "max_rss_mb": (
                            (worker.max_rss_kb / 1024.0)
                            if worker.max_rss_kb is not None
                            else None
                        ),
                        "user_cpu_sec": worker.user_cpu_sec,
                        "system_cpu_sec": worker.system_cpu_sec,
                        "cpu_pct": worker.cpu_pct,
                        "voluntary_ctx_switches": worker.voluntary_ctx_switches,
                        "involuntary_ctx_switches": worker.involuntary_ctx_switches,
                        "peak_open_fds": worker.peak_open_fds,
                        "peak_direct_children": worker.peak_direct_children,
                        "sample_count": worker.sample_count,
                        "sampled_peak_parent_rss_kb": worker.sampled_peak_parent_rss_kb,
                        "sampled_peak_parent_cpu_pct": worker.sampled_peak_parent_cpu_pct,
                        "sampled_peak_tree_rss_kb": worker.sampled_peak_tree_rss_kb,
                        "sampled_peak_tree_rss_mb": (
                            (worker.sampled_peak_tree_rss_kb / 1024.0)
                            if worker.sampled_peak_tree_rss_kb is not None
                            else None
                        ),
                        "sampled_peak_tree_cpu_pct": worker.sampled_peak_tree_cpu_pct,
                        "sampled_mean_tree_cpu_pct": worker.sampled_mean_tree_cpu_pct,
                        "build_cmd_ms": worker.build_cmd_ms,
                        "spawn_proc_ms": worker.spawn_proc_ms,
                        "monitor_loop_ms": worker.monitor_loop_ms,
                        "communicate_ms": worker.communicate_ms,
                        "parse_stats_ms": worker.parse_stats_ms,
                        "top_sample_count": worker.top_sample_count,
                        "top_peak_rss_mb": worker.top_peak_rss_mb,
                        "top_mean_rss_mb": worker.top_mean_rss_mb,
                        "top_peak_cpu_pct": worker.top_peak_cpu_pct,
                        "top_mean_cpu_pct": worker.top_mean_cpu_pct,
                        "vmmap_start_physical_footprint_mb": worker.vmmap_start_physical_footprint_mb,
                        "vmmap_mid_physical_footprint_mb": worker.vmmap_mid_physical_footprint_mb,
                        "vmmap_end_physical_footprint_mb": worker.vmmap_end_physical_footprint_mb,
                        "xctrace_trace_path": worker.xctrace_trace_path,
                        "xctrace_hotspots": worker.xctrace_hotspots,
                    }
                    for worker in result.worker_results
                ]

            raw_path = run_dir / f"iteration-{i:03d}.json"
            raw_path.write_text(json.dumps(iteration_payload, indent=2) + "\n", encoding="utf-8")

        summary = summarize_results(args.cmd, all_results, run_dir, config_path, profile)
        summary_json = run_dir / "summary.json"
        summary_md = run_dir / "summary.md"
        summary_json.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
        write_markdown(summary, summary_md)

        print(f"summary_json={summary_json}")
        print(f"summary_md={summary_md}")

        return 0
    finally:
        server.shutdown()
        server.server_close()
        if args.keep_temp:
            print(f"temp_dir={temp_root}")
        else:
            shutil.rmtree(temp_root, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
