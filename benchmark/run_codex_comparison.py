#!/usr/bin/env python3
"""Compare cod3x vs stock Codex binaries on workloads that rely on LD_LIBRARY_PATH."""

from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional

SCENARIOS: List[Dict[str, object]] = [
    {
        "name": "fastmath-baseline",
        "description": "Custom libfastmath.so shipped with this repo.",
        "primary_lib": "libfastmath.so",
        "extra_libs": [],
        "iterations": 30_000_000,
        "ld_paths": ["{bench}/lib"],
        "notes": "Control case that reproduces the minimal reproduction from FINAL_REPORT.md.",
    },
    {
        "name": "cuda-toolkit",
        "description": "CUFFT/CUBLAS style workload that expects libcublas/libcudart via LD_LIBRARY_PATH.",
        "primary_lib": "libcublas.so",
        "extra_libs": ["libcudart.so", "libcusolver.so"],
        "iterations": 30_000_000,
        "ld_paths": [
            "{bench}/lib",
            "${CUDA_LIB_DIR}",
            "${CUDA_HOME}/lib64",
            "/usr/local/cuda/lib64",
        ],
        "notes": "If CUDA is installed locally, set CUDA_LIB_DIR or CUDA_HOME so the real toolkit is preferred. Otherwise the repo-provided fixture libraries highlight the same env stripping behavior.",
    },
    {
        "name": "intel-mkl",
        "description": "MKL / oneAPI runtimes shipped via LD_LIBRARY_PATH (libmkl_rt).",
        "primary_lib": "libmkl_rt.so",
        "extra_libs": ["libtorch_cuda.so", "libopenblas.so.0"],
        "iterations": 30_000_000,
        "ld_paths": [
            "{bench}/lib",
            "${MKLROOT}/lib/intel64",
            "${ONEAPI_ROOT}/mkl/latest/lib/intel64",
        ],
        "notes": "Conda environments frequently append libmkl_rt.so to LD_LIBRARY_PATH; stripping it forces NumPy/SciPy to fall back to the slow reference BLAS.",
    },
]


def _default_stock_path(bench_root: Path) -> Path:
    candidate = Path.home() / "dev" / "openai-codex" / "codex-rs" / "target" / "release" / "codex"
    return candidate if candidate.exists() else Path("/nonexistent")


def parse_args() -> argparse.Namespace:
    bench_root = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--cod3x-bin",
        type=Path,
        default=bench_root.parent / "codex-rs" / "target" / "release" / "codex",
        help="Path to the cod3x release binary",
    )
    parser.add_argument(
        "--stock-bin",
        type=Path,
        default=_default_stock_path(bench_root),
        help="Path to the upstream openai/codex release binary",
    )
    parser.add_argument(
        "--python-bin",
        type=Path,
        default=Path(sys.executable),
        help="Python interpreter used to run probe scripts",
    )
    parser.add_argument(
        "--results-dir",
        type=Path,
        help="Custom directory for JSON/Markdown outputs",
    )
    parser.add_argument(
        "--scenarios",
        nargs="+",
        default=[scenario["name"] for scenario in SCENARIOS],
        help="Subset of scenarios to execute",
    )
    parser.add_argument(
        "--skip-stock",
        action="store_true",
        help="Only run cod3x (useful for CI smoke tests)",
    )
    return parser.parse_args()


def _scenario_map() -> Dict[str, Dict[str, object]]:
    return {scenario["name"]: scenario for scenario in SCENARIOS}


def _expand_candidate(template: str, bench_root: Path) -> Optional[Path]:
    template = template.replace("{bench}", str(bench_root))
    expanded = os.path.expandvars(template)
    expanded = os.path.expanduser(expanded)
    path = Path(expanded)
    if any(tok in template for tok in ("$", "${")) and expanded == template:
        # Environment variable was not set; treat as missing.
        return None
    return path


def _compute_ld_path(scenario: Dict[str, object], bench_root: Path, environ: Dict[str, str]) -> str:
    entries: List[str] = []
    for template in scenario.get("ld_paths", []):
        candidate = _expand_candidate(template, bench_root)
        if candidate and candidate.exists():
            entries.append(str(candidate))
    if not entries:
        # Fall back to whatever LD_LIBRARY_PATH the caller already set so the probe can still run.
        existing = environ.get("LD_LIBRARY_PATH")
        if existing:
            return existing
        return ""
    existing = environ.get("LD_LIBRARY_PATH")
    if existing:
        entries.append(existing)
    # Preserve order but drop duplicates while keeping empties out.
    seen = set()
    ordered: List[str] = []
    for entry in entries:
        if entry and entry not in seen:
            seen.add(entry)
            ordered.append(entry)
    return ":".join(ordered)


def _ensure_binary(path: Path, label: str) -> Path:
    if not path.exists():
        raise FileNotFoundError(f"{label} binary not found at {path}")
    return path


def _run_probe(
    *,
    binary_label: str,
    binary_path: Path,
    scenario: Dict[str, object],
    bench_root: Path,
    python_bin: Path,
    results_dir: Path,
    environ: Dict[str, str],
) -> Dict[str, object]:
    scenario_dir = results_dir / scenario["name"]
    scenario_dir.mkdir(parents=True, exist_ok=True)
    output_json = scenario_dir / f"{binary_label}.json"
    log_file = scenario_dir / f"{binary_label}.log"

    probe_script = bench_root / "src" / "gpu_blas_probe.py"
    cmd_parts = [
        shlex.quote(str(python_bin)),
        shlex.quote(str(probe_script)),
        "--scenario",
        shlex.quote(str(scenario["name"])),
        "--primary-lib",
        shlex.quote(str(scenario["primary_lib"])),
        "--iterations",
        str(scenario.get("iterations", 5_000_000)),
        "--output",
        shlex.quote(str(output_json)),
    ]
    for extra in scenario.get("extra_libs", []):
        cmd_parts.extend(["--extra-lib", shlex.quote(extra)])
    if note := scenario.get("notes"):
        cmd_parts.extend(["--notes", shlex.quote(note)])

    prompt = (
        "Run exactly this command once and report its stdout verbatim: "
        + " ".join(cmd_parts)
    )

    env = dict(environ)
    env_ld = _compute_ld_path(scenario, bench_root, env)
    if env_ld:
        env["LD_LIBRARY_PATH"] = env_ld
    else:
        env.pop("LD_LIBRARY_PATH", None)

    proc = subprocess.run(
        [str(binary_path), "exec", "--skip-git-repo-check", prompt],
        env=env,
        text=True,
        capture_output=True,
    )
    log_file.write_text(
        f"$ {' '.join(shlex.quote(part) for part in proc.args)}\n"
        f"returncode: {proc.returncode}\n\n"
        f"--- stdout ---\n{proc.stdout}\n"
        f"--- stderr ---\n{proc.stderr}\n",
        encoding="utf-8",
    )
    if not output_json.exists():
        raise RuntimeError(
            f"Probe command for {binary_label}/{scenario['name']} did not produce {output_json}."
        )

    data = json.loads(output_json.read_text(encoding="utf-8"))
    data["returncode"] = proc.returncode
    data["log_file"] = str(log_file)
    return data


def _duration_from(result: Dict[str, object]) -> Optional[float]:
    workload = result.get("workload") or {}
    duration = workload.get("duration_seconds")
    if isinstance(duration, (int, float)):
        return float(duration)
    return None


def main() -> int:
    args = parse_args()
    bench_root = Path(__file__).resolve().parent
    cod3x_bin = _ensure_binary(args.cod3x_bin, "cod3x")
    stock_required = not args.skip_stock
    stock_bin = None
    if stock_required:
        stock_bin = _ensure_binary(args.stock_bin, "stock Codex")

    timestamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
    results_dir = args.results_dir or bench_root / "results" / f"comparison_{timestamp}"
    results_dir.mkdir(parents=True, exist_ok=True)

    scenario_lookup = _scenario_map()
    requested = []
    for name in args.scenarios:
        if name not in scenario_lookup:
            raise KeyError(f"Unknown scenario '{name}'. Available: {', '.join(scenario_lookup)}")
        requested.append(scenario_lookup[name])

    summary: List[Dict[str, object]] = []
    for scenario in requested:
        print(f"\n=== Scenario: {scenario['name']} ===")
        base_env = os.environ.copy()
        cod3x_result = _run_probe(
            binary_label="cod3x",
            binary_path=cod3x_bin,
            scenario=scenario,
            bench_root=bench_root,
            python_bin=args.python_bin,
            results_dir=results_dir,
            environ=base_env,
        )
        summary.append({
            "scenario": scenario["name"],
            "variant": "cod3x",
            "result": cod3x_result,
        })
        if stock_required and stock_bin is not None:
            stock_result = _run_probe(
                binary_label="stock",
                binary_path=stock_bin,
                scenario=scenario,
                bench_root=bench_root,
                python_bin=args.python_bin,
                results_dir=results_dir,
                environ=base_env,
            )
            summary.append({
                "scenario": scenario["name"],
                "variant": "stock",
                "result": stock_result,
            })

    summary_path = results_dir / "comparison_summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")

    # Produce a concise Markdown report.
    lines = ["# Codex pre-main hardening comparison", ""]
    for scenario in requested:
        lines.append(f"## {scenario['name']}")
        lines.append(scenario.get("description", ""))
        lines.append("")
        cod3x = next(item for item in summary if item["scenario"] == scenario["name"] and item["variant"] == "cod3x")
        cod3x_duration = _duration_from(cod3x["result"]) or 0.0
        lines.append(f"- cod3x duration: {cod3x_duration:.3f}s ({cod3x['result']['workload']['mode']})")
        if stock_required:
            stock = next(item for item in summary if item["scenario"] == scenario["name"] and item["variant"] == "stock")
            stock_duration = _duration_from(stock["result"]) or 0.0
            ratio = stock_duration / cod3x_duration if cod3x_duration else float('inf')
            lines.append(
                f"- stock duration: {stock_duration:.3f}s ({stock['result']['workload']['mode']}), slowdown ×{ratio:.1f}"
            )
        if note := scenario.get("notes"):
            lines.append(f"- Notes: {note}")
        lines.append("")
    report_path = results_dir / "comparison_report.md"
    report_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"\nWrote {summary_path} and {report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
