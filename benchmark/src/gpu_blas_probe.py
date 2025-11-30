#!/usr/bin/env python3
"""Probe shared libraries that depend on LD_LIBRARY_PATH and run an optional workload."""

from __future__ import annotations

import argparse
import ctypes
import json
import math
import os
import platform
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List


def _pure_python(iterations: int) -> Dict[str, float]:
    start = time.perf_counter()
    total = 0.0
    for i in range(iterations):
        total += math.sin(i) * math.cos(i) + math.sqrt(i + 1)
    duration = time.perf_counter() - start
    return {"duration_seconds": duration, "result": total}


def _load_shared_library(libname: str) -> ctypes.CDLL:
    """Load a shared library by name, relying on the dynamic loader search path."""
    return ctypes.CDLL(libname)


def _probe_lib(libname: str) -> Dict[str, str]:
    status: Dict[str, str] = {"name": libname}
    try:
        lib = _load_shared_library(libname)
        resolved = getattr(lib, "_name", libname)
        status.update({"status": "loaded", "resolved_path": str(resolved)})
    except OSError as exc:  # pragma: no cover - exercised via benchmark harness
        status.update({"status": "error", "error": str(exc)})
    return status


def _run_workload(libname: str, symbol: str, iterations: int) -> Dict[str, object]:
    if iterations <= 0:
        return {
            "mode": "skipped",
            "reason": "iterations<=0",
            "duration_seconds": 0.0,
        }

    try:
        lib = _load_shared_library(libname)
        resolved = getattr(lib, "_name", libname)
        func = getattr(lib, symbol)
        func.argtypes = [ctypes.c_int]
        start = time.perf_counter()
        func(iterations)
        duration = time.perf_counter() - start
        return {
            "mode": "optimized",
            "duration_seconds": duration,
            "resolved_path": str(resolved),
        }
    except (AttributeError, OSError) as exc:  # pragma: no cover - runtime measured externally
        fallback = _pure_python(iterations)
        return {
            "mode": "fallback",
            "reason": str(exc),
            "duration_seconds": fallback["duration_seconds"],
            "fallback_result": fallback["result"],
        }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Load a primary shared library (defaults to libfastmath.so) and run a "
            "deterministic compute workload. Additional libraries can be probed to "
            "mirror CUDA, MKL, or OpenBLAS dependency chains."
        )
    )
    parser.add_argument("--scenario", default="fastmath", help="label for the current probe")
    parser.add_argument(
        "--primary-lib",
        default="libfastmath.so",
        help="Shared library name used for the timed workload",
    )
    parser.add_argument(
        "--symbol",
        default="compute_intensive_task",
        help="Symbol exported by the primary library to run (default matches fastmath.c)",
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=5_000_000,
        help="Workload iterations that stress the shared library",
    )
    parser.add_argument(
        "--extra-lib",
        action="append",
        default=[],
        help="Additional shared libraries to probe (repeat flag to add more)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Optional JSON output destination",
    )
    parser.add_argument(
        "--notes",
        help="Optional free-form note included in the JSON output",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    ld_path = os.environ.get("LD_LIBRARY_PATH", "<unset>")
    result: Dict[str, object] = {
        "scenario": args.scenario,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "iterations": args.iterations,
        "primary_lib": args.primary_lib,
        "symbol": args.symbol,
        "python_executable": sys.executable,
        "python_version": sys.version,
        "hostname": platform.node(),
        "ld_library_path": ld_path,
    }
    if args.notes:
        result["notes"] = args.notes

    workload = _run_workload(args.primary_lib, args.symbol, args.iterations)
    result["workload"] = workload

    if args.extra_lib:
        result["extra_libs"] = [_probe_lib(lib) for lib in args.extra_lib]

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(f"wrote {args.output}")

    if workload["mode"] == "optimized":
        print(
            f"[optimized] {args.primary_lib} handled {args.iterations:,} iterations "
            f"in {workload['duration_seconds']:.3f}s"
        )
    elif workload["mode"] == "fallback":
        print(
            f"[fallback] {args.primary_lib} unavailable ({workload['reason']}); "
            f"ran Python loop for {workload['duration_seconds']:.3f}s"
        )
    else:
        print("[skipped] workload disabled by configuration")

    if args.extra_lib:
        for lib_result in result["extra_libs"]:
            status = lib_result["status"]
            details = lib_result.get("resolved_path") or lib_result.get("error")
            print(f"[{status}] {lib_result['name']} :: {details}")

    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
