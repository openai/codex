#!/usr/bin/env python3
"""Measure Codex Responses request size for the current checkout.

This is intentionally a black-box harness: it builds the real `codex` CLI from
the current checkout, runs a few `codex exec` scenarios against a local mock
Responses API, and measures the request body the client would have sent to the
backend.

The NDJSON stream is detailed enough for debugging; `--summary-output` writes a
compact artifact that a later GitHub/codex-action reviewer can compare against
baseline data and use to explain whether a PR is causing context regressions.
"""

from __future__ import annotations

import argparse
import contextlib
import dataclasses
import datetime as dt
import http.server
import json
import os
import signal
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Any


HOST = "127.0.0.1"
DEFAULT_SCENARIOS = ("baseline", "resume", "project_instructions", "output_schema", "workspace_write")
SUMMARY_MEASUREMENT_FIELDS = (
    "scenario",
    "run_label",
    "model",
    "request_body_bytes",
    "context_component_bytes",
    "instructions_bytes",
    "input_json_bytes",
    "tools_json_bytes",
    "developer_message_json_bytes",
    "user_message_json_bytes",
    "tool_count",
    "input_item_count",
    "build_elapsed_ms",
    "command_elapsed_ms",
    "command_status",
    "shape_reasons",
)


@dataclasses.dataclass(frozen=True)
class CommandResult:
    returncode: int
    stdout: str
    stderr: str
    elapsed_ms: int
    timed_out: bool = False


class ResponsesHandler(http.server.BaseHTTPRequestHandler):
    server: Any

    def log_message(self, fmt: str, *args: object) -> None:
        return

    def do_GET(self) -> None:
        self._send_empty_json()

    def do_POST(self) -> None:
        length = int(self.headers.get("content-length", "0"))
        raw_body = self.rfile.read(length)
        path = self.path.split("?", 1)[0]

        if path.endswith("/responses"):
            self.server.captured.append(
                {
                    "path": path,
                    "headers": {key.lower(): value for key, value in self.headers.items()},
                    "raw_body": raw_body,
                }
            )
            self._send_sse()
            return

        self._send_empty_json()

    def _send_empty_json(self) -> None:
        body = b"{}"
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_sse(self) -> None:
        body = b'event: response.completed\ndata: {"type":"response.completed","response":{"id":"resp"}}\n\n'
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("cache-control", "no-cache")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


@contextlib.contextmanager
def mock_responses_server() -> Any:
    server = http.server.ThreadingHTTPServer((HOST, 0), ResponsesHandler)
    server.captured = []
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def run_command(
    args: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    timeout_seconds: int,
) -> CommandResult:
    start = time.monotonic()
    proc = subprocess.Popen(
        args,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    timed_out = False
    try:
        stdout, stderr = proc.communicate(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        timed_out = True
        with contextlib.suppress(ProcessLookupError):
            os.killpg(proc.pid, signal.SIGTERM)
        try:
            stdout, stderr = proc.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            with contextlib.suppress(ProcessLookupError):
                os.killpg(proc.pid, signal.SIGKILL)
            stdout, stderr = proc.communicate()
    elapsed_ms = int((time.monotonic() - start) * 1000)
    return CommandResult(proc.returncode, stdout, stderr, elapsed_ms, timed_out)


def tail(text: str, max_chars: int = 4000) -> str:
    if len(text) <= max_chars:
        return text
    return text[-max_chars:]


def compact_json(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def byte_len_text(value: str | None) -> int:
    return len((value or "").encode("utf-8"))


def request_metrics(request: dict[str, Any]) -> tuple[dict[str, Any], list[str]]:
    reasons: list[str] = []
    body_json: Any = None
    raw_body = request["raw_body"]
    headers = request["headers"]
    encoding = headers.get("content-encoding")
    if encoding:
        reasons.append(f"unsupported content-encoding: {encoding}")
    else:
        try:
            body_json = json.loads(raw_body)
        except json.JSONDecodeError as err:
            reasons.append(f"request body is not JSON: {err}")

    metrics: dict[str, Any] = {
        "request_path": request["path"],
        "request_body_bytes": len(raw_body),
    }
    if not isinstance(body_json, dict):
        return metrics, reasons

    instructions = body_json.get("instructions")
    input_value = body_json.get("input")
    tools_value = body_json.get("tools")
    messages = input_value if isinstance(input_value, list) else []
    developer_messages = [item for item in messages if item.get("role") in ("developer", "system")]
    user_messages = [item for item in messages if item.get("role") == "user"]

    if "input" not in body_json:
        reasons.append("request JSON has no `input` field")
    if "model" not in body_json:
        reasons.append("request JSON has no `model` field")

    metrics.update(
        {
            "model": body_json.get("model"),
            "instructions_bytes": byte_len_text(instructions if isinstance(instructions, str) else None),
            "input_json_bytes": len(compact_json(input_value)) if input_value is not None else 0,
            "tools_json_bytes": len(compact_json(tools_value)) if tools_value is not None else 0,
            "context_component_bytes": (
                byte_len_text(instructions if isinstance(instructions, str) else None)
                + (len(compact_json(input_value)) if input_value is not None else 0)
                + (len(compact_json(tools_value)) if tools_value is not None else 0)
            ),
            "input_item_count": len(messages),
            "developer_message_count": len(developer_messages),
            "developer_message_json_bytes": len(compact_json(developer_messages)),
            "user_message_count": len(user_messages),
            "user_message_json_bytes": len(compact_json(user_messages)),
            "tool_count": len(tools_value) if isinstance(tools_value, list) else 0,
        }
    )
    return metrics, reasons


def write_scenario_files(scenario: str, workspace: Path, home: Path) -> None:
    workspace.mkdir(parents=True, exist_ok=True)
    home.mkdir(parents=True, exist_ok=True)
    if scenario == "project_instructions":
        (workspace / "AGENTS.md").write_text(
            "\n".join(
                [
                    "# Project Instructions",
                    "",
                    "- Treat this workspace as a context-bloat measurement fixture.",
                    "- Prefer concise answers.",
                    "- Mention the fixture marker `context-bloat-project-doc` if asked about project policy.",
                    "- Do not run shell commands unless explicitly requested.",
                    "- Keep generated output deterministic for comparison.",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
    if scenario == "output_schema":
        (workspace / "schema.json").write_text(
            json.dumps(
                {
                    "type": "object",
                    "additionalProperties": False,
                    "properties": {"answer": {"type": "string"}},
                    "required": ["answer"],
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )


def codex_env(home: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["CODEX_HOME"] = str(home)
    env["OPENAI_API_KEY"] = "dummy"
    env["NO_COLOR"] = "1"
    env.pop("CODEX_SANDBOX_NETWORK_DISABLED", None)
    env.pop("CODEX_SANDBOX", None)
    return env


def base_exec_args(server_url: str, workspace: Path, extra_config: list[str]) -> list[str]:
    provider_override = (
        f'model_providers.mock={{ name = "mock", base_url = "{server_url}/v1", '
        f'env_key = "OPENAI_API_KEY", wire_api = "responses" }}'
    )
    args = [
        "exec",
        "--skip-git-repo-check",
        "-c",
        provider_override,
        "-c",
        'model_provider="mock"',
        "-c",
        f'chatgpt_base_url="{server_url}/backend-api"',
    ]
    for config in extra_config:
        args.extend(["-c", config])
    args.extend(["-C", str(workspace)])
    return args


def run_scenario(
    *,
    binary: Path,
    scenario: str,
    scenario_root: Path,
    run_timeout_seconds: int,
    extra_config: list[str],
) -> tuple[list[dict[str, Any]], list[str]]:
    workspace = scenario_root / "workspace"
    home = scenario_root / "home"
    write_scenario_files(scenario, workspace, home)
    env = codex_env(home)
    prompt = f"Reply with exactly `done` for scenario {scenario}."
    measurements: list[dict[str, Any]] = []
    reasons: list[str] = []

    with mock_responses_server() as server:
        actual_server_url = f"http://{HOST}:{server.server_address[1]}"
        common = base_exec_args(actual_server_url, workspace, extra_config)
        if scenario == "workspace_write":
            common.extend(["--sandbox", "workspace-write"])
        if scenario == "output_schema":
            common.extend(["--output-schema", str(workspace / "schema.json")])

        first = run_command(
            [str(binary), *common, prompt],
            cwd=workspace,
            env=env,
            timeout_seconds=run_timeout_seconds,
        )
        measurements.extend(
            collect_new_measurements(
                server,
                scenario=scenario,
                run_label="first_turn",
                command_result=first,
            )
        )
        if first.returncode != 0:
            reasons.append(f"{scenario} first turn failed: {tail(first.stderr or first.stdout)}")
            return measurements, reasons

        if scenario == "resume":
            resume_prompt = "Reply with exactly `done` for the resumed turn."
            resume_args = [*common, "resume", "--last", resume_prompt]
            second = run_command(
                [str(binary), *resume_args],
                cwd=workspace,
                env=env,
                timeout_seconds=run_timeout_seconds,
            )
            new_measurements = collect_new_measurements(
                server,
                scenario=scenario,
                run_label="second_turn",
                command_result=second,
            )
            measurements.extend(new_measurements)
            if second.returncode != 0:
                reasons.append(f"resume second turn failed: {tail(second.stderr or second.stdout)}")
            if not new_measurements:
                reasons.append("resume second turn did not capture a Responses request")

    if not measurements:
        reasons.append(f"{scenario} did not capture any Responses requests")
    return measurements, reasons


def collect_new_measurements(
    server: Any,
    *,
    scenario: str,
    run_label: str,
    command_result: CommandResult,
) -> list[dict[str, Any]]:
    requests = list(server.captured)
    server.captured.clear()
    rows: list[dict[str, Any]] = []
    for index, request in enumerate(requests):
        metrics, shape_reasons = request_metrics(request)
        rows.append(
            {
                "record_type": "measurement",
                "scenario": scenario,
                "run_label": run_label,
                "request_index": index,
                "command_elapsed_ms": command_result.elapsed_ms,
                "command_status": command_result.returncode,
                "command_timed_out": command_result.timed_out,
                "shape_reasons": shape_reasons,
                **metrics,
            }
        )
    return rows


def clean_build_dir(target_dir: Path, *, clean: bool) -> dict[str, Any]:
    start = time.monotonic()
    existed = target_dir.exists()
    skipped_reason = None
    if clean and existed:
        shutil.rmtree(target_dir)
    elif not clean:
        skipped_reason = "custom --target-dir is not cleaned"
    target_dir.mkdir(parents=True, exist_ok=True)
    elapsed_ms = int((time.monotonic() - start) * 1000)
    return {
        "record_type": "cleanup",
        "path": str(target_dir),
        "removed": clean and existed,
        "skipped_reason": skipped_reason,
        "cleanup_elapsed_ms": elapsed_ms,
    }


def build_codex(
    repo: Path,
    *,
    target_dir: Path,
    timeout_seconds: int,
    locked: bool,
) -> tuple[Path | None, CommandResult]:
    manifest = repo / "codex-rs/Cargo.toml"
    args = [
        "cargo",
        "build",
        "--manifest-path",
        str(manifest),
        "-p",
        "codex-cli",
        "--bin",
        "codex",
    ]
    if locked:
        args.append("--locked")
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target_dir)
    result = run_command(args, cwd=repo / "codex-rs", env=env, timeout_seconds=timeout_seconds)
    binary = target_dir / "debug" / "codex"
    if result.returncode == 0 and binary.exists():
        return binary, result
    return None, result


def run_probe(
    *,
    repo: Path,
    work_dir: Path,
    target_dir: Path,
    clean_build: bool,
    scenarios: list[str],
    build_timeout_seconds: int,
    run_timeout_seconds: int,
    locked: bool,
    extra_config: list[str],
    emit: Any,
) -> dict[str, Any]:
    run_root = work_dir / "runs" / str(time.time_ns())
    cleanup_record = clean_build_dir(target_dir, clean=clean_build)
    emit(cleanup_record)
    binary, build = build_codex(repo, target_dir=target_dir, timeout_seconds=build_timeout_seconds, locked=locked)
    build_record = {
        "record_type": "build",
        "status": build.returncode,
        "build_elapsed_ms": build.elapsed_ms,
        "timed_out": build.timed_out,
        "stderr_tail": tail(build.stderr),
    }
    emit(build_record)
    if binary is None:
        summary = make_summary(
            repo=repo,
            cleanup_record=cleanup_record,
            build_record=build_record,
            scenarios=scenarios,
            measurements=[],
            invalid_reasons=[f"build failed: {tail(build.stderr or build.stdout)}"],
        )
        emit(probe_summary_record(summary))
        return summary

    all_measurements: list[dict[str, Any]] = []
    scenario_reasons: list[str] = []
    for scenario in scenarios:
        scenario_measurements, reasons = run_scenario(
            binary=binary,
            scenario=scenario,
            scenario_root=run_root / scenario,
            run_timeout_seconds=run_timeout_seconds,
            extra_config=extra_config,
        )
        for row in scenario_measurements:
            row["build_elapsed_ms"] = build.elapsed_ms
            emit(row)
        all_measurements.extend(scenario_measurements)
        scenario_reasons.extend(reasons)

    shape_reasons = [
        reason
        for row in all_measurements
        for reason in row.get("shape_reasons", [])
        if reason
    ]
    invalid_reasons = [*scenario_reasons, *shape_reasons]
    has_baseline = any(
        row.get("scenario") == "baseline"
        and row.get("request_path", "").endswith("/responses")
        and row.get("input_json_bytes", 0) > 0
        for row in all_measurements
    )
    if not has_baseline:
        invalid_reasons.append("no usable baseline /responses measurement")
    summary = make_summary(
        repo=repo,
        cleanup_record=cleanup_record,
        build_record=build_record,
        scenarios=scenarios,
        measurements=all_measurements,
        invalid_reasons=invalid_reasons,
    )
    emit(probe_summary_record(summary))
    return summary


def make_summary(
    *,
    repo: Path,
    cleanup_record: dict[str, Any],
    build_record: dict[str, Any],
    scenarios: list[str],
    measurements: list[dict[str, Any]],
    invalid_reasons: list[str],
) -> dict[str, Any]:
    return {
        "generated_at": dt.datetime.now(tz=dt.timezone.utc).isoformat(timespec="seconds"),
        "repo": str(repo),
        "valid": not invalid_reasons,
        "invalid_reasons": invalid_reasons,
        "cleanup": cleanup_record,
        "build": build_record,
        "build_elapsed_ms": build_record.get("build_elapsed_ms"),
        "scenarios": scenarios,
        "measurement_count": len(measurements),
        "measurements": [
            {field: row.get(field) for field in SUMMARY_MEASUREMENT_FIELDS if field in row}
            for row in measurements
        ],
    }


def probe_summary_record(summary: dict[str, Any]) -> dict[str, Any]:
    return {
        "record_type": "probe_summary",
        "valid": summary["valid"],
        "invalid_reasons": summary["invalid_reasons"],
        "build_elapsed_ms": summary["build_elapsed_ms"],
        "measurement_count": summary["measurement_count"],
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--work-dir", type=Path, default=Path("/tmp/codex-context-bloat-probe"))
    parser.add_argument(
        "--target-dir",
        type=Path,
        help="Shared Cargo target dir. Defaults under --work-dir. Custom target dirs are not cleaned.",
    )
    parser.add_argument("--scenario", action="append", choices=DEFAULT_SCENARIOS, help="Scenario to run. Repeatable.")
    parser.add_argument("--build-timeout-seconds", type=int, default=1800)
    parser.add_argument("--run-timeout-seconds", type=int, default=120)
    parser.add_argument("--cargo-locked", action="store_true", help="Pass --locked to cargo build.")
    parser.add_argument("--output", type=Path, help="Write NDJSON records to this file instead of stdout.")
    parser.add_argument("--summary-output", type=Path, help="Write a compact JSON summary for CI/Codex review.")
    parser.add_argument(
        "--fail-on-invalid",
        action="store_true",
        help="Exit non-zero if the probe cannot capture a valid baseline measurement.",
    )
    parser.add_argument(
        "-c",
        "--config",
        action="append",
        default=[],
        help="Extra Codex config override passed through to `codex exec -c`.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo = args.repo.resolve()
    work_dir = args.work_dir.resolve()
    target_dir = (args.target_dir or work_dir / "target").resolve()
    clean_build = args.target_dir is None
    scenarios = args.scenario or list(DEFAULT_SCENARIOS)
    work_dir.mkdir(parents=True, exist_ok=True)

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
    out = args.output.open("w", encoding="utf-8") if args.output else sys.stdout

    def emit(record: dict[str, Any]) -> None:
        print(json.dumps(record, ensure_ascii=False, sort_keys=True), file=out, flush=True)

    try:
        emit(
            {
                "record_type": "probe_start",
                "scenarios": scenarios,
            }
        )
        summary = run_probe(
            repo=repo,
            work_dir=work_dir,
            target_dir=target_dir,
            clean_build=clean_build,
            scenarios=scenarios,
            build_timeout_seconds=args.build_timeout_seconds,
            run_timeout_seconds=args.run_timeout_seconds,
            locked=args.cargo_locked,
            extra_config=args.config,
            emit=emit,
        )
        if args.summary_output:
            args.summary_output.parent.mkdir(parents=True, exist_ok=True)
            args.summary_output.write_text(
                json.dumps(summary, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
        if args.fail_on_invalid and not summary["valid"]:
            return 1
        return 0
    finally:
        if args.output:
            out.close()


if __name__ == "__main__":
    raise SystemExit(main())
