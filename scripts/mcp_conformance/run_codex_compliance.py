#!/usr/bin/env python3
"""Black-box MCP compliance runner for a supplied Codex binary."""

import argparse
import importlib.util
import json
import os
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from collections import deque
from contextlib import contextmanager, nullcontext
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable, Iterator, Mapping, Sequence

_MODULE_DIR = Path(__file__).resolve().parent
if str(_MODULE_DIR) not in sys.path:
    # Some execution environments set PYTHONSAFEPATH. Add only this
    # trusted package directory so direct `python path/to/script.py` usage
    # continues to work alongside the Bazel entry point.
    sys.path.insert(0, str(_MODULE_DIR))

from official_conformance import (  # noqa: E402 - direct scripts must first add their sibling directory.
    OFFICIAL_CONFORMANCE_GIT_REF,
    OFFICIAL_CONFORMANCE_REPOSITORY,
    OfficialScenarioResult,
    default_conformance_command,
    run_official_mode,
    scenarios_for_mode,
)

_FIXTURE_MODULE_PATH = Path(__file__).resolve().with_name("server.py")
_FIXTURE_SPEC = importlib.util.spec_from_file_location(
    "_mcp_spec_test_fixture_server",
    _FIXTURE_MODULE_PATH,
)
if _FIXTURE_SPEC is None or _FIXTURE_SPEC.loader is None:
    raise RuntimeError(f"could not load MCP fixture module: {_FIXTURE_MODULE_PATH}")
_FIXTURE_MODULE = importlib.util.module_from_spec(_FIXTURE_SPEC)
sys.modules[_FIXTURE_SPEC.name] = _FIXTURE_MODULE
_FIXTURE_SPEC.loader.exec_module(_FIXTURE_MODULE)

LEGACY_VERSION = _FIXTURE_MODULE.LEGACY_VERSION
SHIPPING_LEGACY_VERSION = _FIXTURE_MODULE.SHIPPING_LEGACY_VERSION
MODERN_VERSION = _FIXTURE_MODULE.MODERN_VERSION
MISMATCHED_DISCOVERY_ID_PROFILE = _FIXTURE_MODULE.MISMATCHED_DISCOVERY_ID_PROFILE
NULL_DISCOVERY_ID_PROFILE = _FIXTURE_MODULE.NULL_DISCOVERY_ID_PROFILE
REPEATED_CURSOR_PROFILE = _FIXTURE_MODULE.REPEATED_CURSOR_PROFILE
REVIEW_EXACT_INTEGER = _FIXTURE_MODULE.REVIEW_EXACT_INTEGER
REVIEW_MRTR_INPUT_REQUEST_COUNT = _FIXTURE_MODULE.REVIEW_MRTR_INPUT_REQUEST_COUNT
REVIEW_PROFILE = _FIXTURE_MODULE.REVIEW_PROFILE
RESOURCE_URIS = _FIXTURE_MODULE.RESOURCE_URIS
SERVER_NAME = _FIXTURE_MODULE.SERVER_NAME
SERVER_VERSION = _FIXTURE_MODULE.SERVER_VERSION
SSE_COMMENT_FLOOD_PROFILE = _FIXTURE_MODULE.SSE_COMMENT_FLOOD_PROFILE
SSE_CR_COMMENTS_PROFILE = _FIXTURE_MODULE.SSE_CR_COMMENTS_PROFILE
ProtocolServer = _FIXTURE_MODULE.ProtocolServer
make_http_server = _FIXTURE_MODULE.make_http_server

REPORT_SCHEMA_VERSION = 4
COMPACT_REGRESSION_BASELINE_KIND = "mcp-conformance-regression-baseline-v1"
REQUIRED_REGRESSION_MODES = (
    SHIPPING_LEGACY_VERSION,
    LEGACY_VERSION,
    MODERN_VERSION,
)
TEST_SERVER_NAME = "mcp_spec_fixture"
TEST_META_KEY = "com.openai/mcp-spec-test"
CHECK_ORDER = (
    "mcp_add",
    "mcp_get",
    "app_server_initialize",
    "modern_feature_enablement",
    "inventory",
    "ephemeral_thread",
    "echo_tool",
    "unicode_resource_read",
    "per_request_metadata",
    "request_scoped_notifications",
    "multi_round_trip_request",
    "http_header_mirroring",
    "app_server_protocol",
    "case_runtime",
    "mcp_remove",
    "isolated_config_cleanup",
)


@dataclass
class CommandResult:
    returncode: int
    stdout: str
    stderr: str


@dataclass
class CheckResult:
    name: str
    success: bool
    detail: str
    status: str | None = None
    source: str = "supplemental"
    scenario: str | None = None
    check_id: str | None = None
    category: str | None = None

    def __post_init__(self) -> None:
        if self.status is None:
            self.status = "PASS" if self.success else "FAIL"


@dataclass
class CaseResult:
    transport: str
    mode: str
    success: bool = False
    duration_seconds: float = 0.0
    checks: list[CheckResult] = field(default_factory=list)
    diagnostics: str | None = None

    def check(self, name: str, success: bool, detail: str) -> bool:
        self.checks.append(
            CheckResult(
                name=name,
                success=success,
                detail=detail,
                status="PASS" if success else "FAIL",
            )
        )
        return success

    def finish(self, started_at: float) -> None:
        self.duration_seconds = round(time.monotonic() - started_at, 3)
        self.success = bool(self.checks) and all(check.success for check in self.checks)


@dataclass(frozen=True, order=True)
class _RegressionCheckIdentity:
    mode: str
    transport: str
    source: str
    scenario: str
    check_id: str


class AppServerError(RuntimeError):
    pass


class AppServerClient:
    """Minimal JSONL client for the Codex app-server protocol."""

    def __init__(
        self,
        codex_binary: Path,
        *,
        env: Mapping[str, str],
        cwd: Path,
        timeout_seconds: float,
        elicitation_content: (
            Callable[[Mapping[str, object]], Mapping[str, object]] | None
        ) = None,
    ) -> None:
        self.timeout_seconds = timeout_seconds
        self._elicitation_content = elicitation_content
        self._next_id = 1
        self._messages: queue.Queue[dict[str, object] | None] = queue.Queue()
        self._write_lock = threading.Lock()
        self._stderr: deque[str] = deque(maxlen=200)
        self._parse_errors: deque[str] = deque(maxlen=20)
        self.events: list[dict[str, object]] = []
        self.elicitation_requests: list[dict[str, object]] = []
        self.process = subprocess.Popen(
            [str(codex_binary), "app-server"],
            cwd=cwd,
            env=dict(env),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self._stdout_thread = threading.Thread(
            target=self._read_stdout,
            name="codex-app-server-stdout",
            daemon=True,
        )
        self._stderr_thread = threading.Thread(
            target=self._read_stderr,
            name="codex-app-server-stderr",
            daemon=True,
        )
        self._stdout_thread.start()
        self._stderr_thread.start()

    def __enter__(self) -> "AppServerClient":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def _read_stdout(self) -> None:
        assert self.process.stdout is not None
        for line in self.process.stdout:
            stripped = line.strip()
            if not stripped:
                continue
            try:
                message = json.loads(stripped)
            except json.JSONDecodeError:
                self._parse_errors.append(stripped[:500])
                continue
            if isinstance(message, dict):
                self._messages.put(message)
        self._messages.put(None)

    def _read_stderr(self) -> None:
        assert self.process.stderr is not None
        for line in self.process.stderr:
            self._stderr.append(line.rstrip())

    def _send(self, message: Mapping[str, object]) -> None:
        if self.process.poll() is not None:
            raise AppServerError(
                f"Codex app-server exited with code {self.process.returncode}"
            )
        assert self.process.stdin is not None
        encoded = json.dumps(message, ensure_ascii=False, separators=(",", ":"))
        with self._write_lock:
            self.process.stdin.write(encoded + "\n")
            self.process.stdin.flush()

    def notify(self, method: str) -> None:
        self._send({"method": method})

    def request(
        self,
        method: str,
        params: Mapping[str, object] | None,
    ) -> dict[str, object]:
        request_id = self._next_id
        self._next_id += 1
        message: dict[str, object] = {"id": request_id, "method": method}
        if params is not None:
            message["params"] = dict(params)
        self._send(message)

        deadline = time.monotonic() + self.timeout_seconds
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise AppServerError(
                    f"timed out waiting for app-server response to {method}"
                )
            try:
                message = self._messages.get(timeout=remaining)
            except queue.Empty as exc:
                raise AppServerError(
                    f"timed out waiting for app-server response to {method}"
                ) from exc
            if message is None:
                raise AppServerError(
                    f"Codex app-server closed stdout while handling {method}"
                )
            if message.get("id") == request_id and "method" not in message:
                return message
            if "id" in message and isinstance(message.get("method"), str):
                self._handle_server_request(message)
            else:
                self.events.append(message)

    def wait_for_notification(
        self,
        method: str,
        *,
        predicate: Callable[[Mapping[str, object]], bool] | None = None,
        after_event_index: int = 0,
    ) -> dict[str, object]:
        for event in self.events[after_event_index:]:
            if event.get("method") != method:
                continue
            params = event.get("params")
            if isinstance(params, dict) and (predicate is None or predicate(params)):
                return event

        deadline = time.monotonic() + self.timeout_seconds
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise AppServerError(
                    f"timed out waiting for app-server notification {method}"
                )
            try:
                message = self._messages.get(timeout=remaining)
            except queue.Empty as exc:
                raise AppServerError(
                    f"timed out waiting for app-server notification {method}"
                ) from exc
            if message is None:
                raise AppServerError(
                    f"Codex app-server closed stdout while waiting for {method}"
                )
            if "id" in message and isinstance(message.get("method"), str):
                self._handle_server_request(message)
                continue
            self.events.append(message)
            if message.get("method") != method:
                continue
            params = message.get("params")
            if isinstance(params, dict) and (predicate is None or predicate(params)):
                return message

    def _handle_server_request(self, message: dict[str, object]) -> None:
        self.events.append(message)
        request_id = message.get("id")
        method = message.get("method")
        if method == "mcpServer/elicitation/request":
            params = message.get("params")
            if isinstance(params, dict):
                self.elicitation_requests.append(params)
            else:
                params = {}
            content = (
                dict(self._elicitation_content(params))
                if self._elicitation_content is not None
                else {"confirmation": "confirmed"}
            )
            self._send(
                {
                    "id": request_id,
                    "result": {
                        "action": "accept",
                        "content": content,
                        "_meta": None,
                    },
                }
            )
            return
        self._send(
            {
                "id": request_id,
                "error": {
                    "code": -32601,
                    "message": f"unsupported test client request: {method}",
                },
            }
        )

    def diagnostic_text(self) -> str:
        parts: list[str] = []
        if self._parse_errors:
            parts.append("non-JSON stdout: " + " | ".join(self._parse_errors))
        startup_failures = [
            event
            for event in self.events
            if event.get("method") == "mcpServer/startupStatus/updated"
            and isinstance(event.get("params"), dict)
            and event["params"].get("status") not in ("starting", "ready")
        ]
        if startup_failures:
            parts.append(
                "startup events: "
                + json.dumps(startup_failures[-3:], ensure_ascii=False)
            )
        if self._stderr:
            parts.append("stderr:\n" + "\n".join(self._stderr))
        return "\n".join(parts)[-8_000:]

    def close(self) -> None:
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=3)
        for stream in (self.process.stdin, self.process.stdout, self.process.stderr):
            if stream is not None:
                stream.close()
        self._stdout_thread.join(timeout=1)
        self._stderr_thread.join(timeout=1)


def _run_command(
    command: Sequence[str],
    *,
    env: Mapping[str, str],
    cwd: Path,
    timeout_seconds: float,
) -> CommandResult:
    try:
        completed = subprocess.run(
            list(command),
            cwd=cwd,
            env=dict(env),
            text=True,
            capture_output=True,
            timeout=timeout_seconds,
            check=False,
        )
        return CommandResult(
            completed.returncode,
            completed.stdout.strip(),
            completed.stderr.strip(),
        )
    except subprocess.TimeoutExpired as exc:
        stdout = (
            exc.stdout.decode() if isinstance(exc.stdout, bytes) else exc.stdout or ""
        )
        stderr = (
            exc.stderr.decode() if isinstance(exc.stderr, bytes) else exc.stderr or ""
        )
        return CommandResult(
            124,
            stdout.strip(),
            (stderr + f"\ncommand timed out after {timeout_seconds:g}s").strip(),
        )


def _response_result(
    response: Mapping[str, object],
) -> tuple[dict[str, object] | None, str]:
    error = response.get("error")
    if isinstance(error, dict):
        return None, json.dumps(error, ensure_ascii=False, sort_keys=True)
    result = response.get("result")
    if not isinstance(result, dict):
        return None, "response did not contain an object result"
    return result, "ok"


def _command_detail(result: CommandResult) -> str:
    detail = result.stdout or result.stderr or f"exit code {result.returncode}"
    return detail[-2_000:]


def _result_detail(
    result: Mapping[str, object] | None,
    fallback: str,
) -> str:
    if result is None:
        return fallback
    return (
        "unexpected result: "
        + json.dumps(result, ensure_ascii=False, sort_keys=True, default=str)
    )[-2_000:]


def _isolated_environment(codex_home: Path) -> dict[str, str]:
    env = dict(os.environ)
    env["CODEX_HOME"] = str(codex_home)
    # Direct app-server MCP calls do not need a model or credentials.
    for name in ("CODEX_API_KEY", "CODEX_ACCESS_TOKEN", "OPENAI_API_KEY"):
        env.pop(name, None)
    return env


@contextmanager
def _running_http_fixture(mode: str) -> Iterator[str]:
    httpd = make_http_server(
        ProtocolServer(mode),
        "127.0.0.1",
        0,
        log_requests=False,
    )
    thread = threading.Thread(
        target=httpd.serve_forever,
        name=f"mcp-fixture-{mode}",
        daemon=True,
    )
    thread.start()
    try:
        _, port = httpd.server_address
        yield f"http://127.0.0.1:{port}/mcp"
    finally:
        httpd.shutdown()
        httpd.server_close()
        thread.join(timeout=3)


def _registration_command(
    codex_binary: Path,
    server_script: Path,
    *,
    transport: str,
    mode: str,
    http_url: str | None,
) -> list[str]:
    command = [str(codex_binary), "mcp", "add", TEST_SERVER_NAME]
    if transport == "stdio":
        if mode == MODERN_VERSION:
            command.extend(["--env", f"CODEX_MCP_PROTOCOL_VERSION={MODERN_VERSION}"])
        return [
            *command,
            "--",
            sys.executable,
            str(server_script),
            "--mode",
            mode,
            "--transport",
            "stdio",
        ]
    assert http_url is not None
    return [*command, "--url", http_url]


def _validate_registration(
    config: Mapping[str, object],
    *,
    transport: str,
    mode: str,
    http_url: str | None,
) -> tuple[bool, str]:
    if config.get("name") != TEST_SERVER_NAME or config.get("enabled") is not True:
        return False, "registered server name or enabled state was incorrect"
    value = config.get("transport")
    if not isinstance(value, dict):
        return False, "registered server did not contain a transport object"
    if transport == "stdio":
        args = value.get("args")
        if value.get("type") != "stdio" or not isinstance(args, list):
            return False, f"unexpected stdio transport: {value!r}"
        if mode not in args or "stdio" not in args:
            return False, f"stdio registration omitted mode or transport: {args!r}"
        env = value.get("env")
        modern_opt_in = (
            isinstance(env, dict)
            and env.get("CODEX_MCP_PROTOCOL_VERSION") == MODERN_VERSION
        )
        if mode == MODERN_VERSION and not modern_opt_in:
            return False, "modern stdio registration omitted its protocol opt-in"
        if mode in (SHIPPING_LEGACY_VERSION, LEGACY_VERSION) and modern_opt_in:
            return (
                False,
                "legacy stdio registration unexpectedly enabled the modern protocol",
            )
    elif value.get("type") != "streamable_http" or value.get("url") != http_url:
        return False, f"unexpected HTTP transport: {value!r}"
    return True, f"registered {value.get('type')} transport"


def _validate_inventory(
    result: Mapping[str, object],
    *,
    mode: str,
) -> tuple[bool, str]:
    entries = result.get("data")
    if not isinstance(entries, list):
        return False, "mcpServerStatus/list result did not contain data"
    entry = next(
        (
            item
            for item in entries
            if isinstance(item, dict) and item.get("name") == TEST_SERVER_NAME
        ),
        None,
    )
    if not isinstance(entry, dict):
        return False, f"{TEST_SERVER_NAME!r} was absent from MCP status"

    missing: list[str] = []
    server_info = entry.get("serverInfo")
    if (
        not isinstance(server_info, dict)
        or server_info.get("name") != SERVER_NAME
        or server_info.get("version") != SERVER_VERSION
    ):
        missing.append("server identity")
    tools = entry.get("tools")
    expected_tools = {"echo", "client_metadata", "progress"}
    if mode == MODERN_VERSION:
        expected_tools.add("request_input")
    if not isinstance(tools, dict):
        missing.append("tool inventory")
    else:
        absent_tools = sorted(expected_tools - set(tools))
        if absent_tools:
            missing.append("tools " + ", ".join(absent_tools))
    resources = entry.get("resources")
    resource_uris = (
        {
            item.get("uri")
            for item in resources
            if isinstance(item, dict) and isinstance(item.get("uri"), str)
        }
        if isinstance(resources, list)
        else set()
    )
    absent_resources = sorted(set(RESOURCE_URIS) - resource_uris)
    if absent_resources:
        missing.append("paginated resources " + ", ".join(absent_resources))
    if missing:
        return False, "missing " + "; ".join(missing)
    return True, "identity, tools, and both paginated resources discovered"


def _validate_echo(
    result: Mapping[str, object],
    sentinel: str,
) -> tuple[bool, str]:
    structured = result.get("structuredContent")
    if isinstance(structured, dict) and structured.get("text") == sentinel:
        return True, "echo returned the exact sentinel"
    return False, f"unexpected echo result: {dict(result)!r}"


def _ratio(passed: int, total: int) -> dict[str, int | float]:
    percentage = round((passed / total) * 100, 1) if total else 0.0
    return {
        "passed": passed,
        "total": total,
        "percentage": percentage,
    }


def _check_ratio(checks: Sequence[CheckResult]) -> dict[str, int | float]:
    applicable = [check for check in checks if check.status != "SKIP"]
    return _ratio(
        sum(check.success for check in applicable),
        len(applicable),
    )


def _scenario_ratio(
    checks: Sequence[CheckResult],
    *,
    category: str,
) -> dict[str, int | float]:
    by_scenario: dict[str, list[CheckResult]] = {}
    for check in checks:
        if (
            check.scenario is None
            or check.category != category
            or check.source not in {"official", "harness"}
        ):
            continue
        by_scenario.setdefault(check.scenario, []).append(check)
    return _ratio(
        sum(
            all(check.success for check in scenario_checks)
            for scenario_checks in by_scenario.values()
        ),
        len(by_scenario),
    )


def _summarize_modes(cases: Sequence[CaseResult]) -> list[dict[str, object]]:
    summaries: list[dict[str, object]] = []
    modes = dict.fromkeys(case.mode for case in cases)
    for mode in modes:
        mode_cases = [case for case in cases if case.mode == mode]
        checks = [check for case in mode_cases for check in case.checks]
        official_checks = [check for check in checks if check.source == "official"]
        official_non_auth_checks = [
            check for check in official_checks if check.category == "non-auth"
        ]
        official_auth_checks = [
            check for check in official_checks if check.category == "auth"
        ]
        harness_checks = [check for check in checks if check.source == "harness"]
        supplemental_checks = [
            check for check in checks if check.source == "supplemental"
        ]
        summaries.append(
            {
                "mode": mode,
                "checks": _check_ratio(checks),
                "officialChecks": _check_ratio(official_checks),
                "officialNonAuthChecks": _check_ratio(official_non_auth_checks),
                "officialAuthChecks": _check_ratio(official_auth_checks),
                "officialNonAuthScenarios": _scenario_ratio(
                    checks,
                    category="non-auth",
                ),
                "officialAuthScenarios": _scenario_ratio(
                    checks,
                    category="auth",
                ),
                "harnessChecks": _check_ratio(harness_checks),
                "supplementalChecks": _check_ratio(supplemental_checks),
                "transportCases": _ratio(
                    sum(case.success for case in mode_cases),
                    len(mode_cases),
                ),
            }
        )
    return summaries


def _build_test_matrix(cases: Sequence[CaseResult]) -> dict[str, object]:
    columns = [
        {
            "key": f"{case.transport}:{case.mode}",
            "transport": case.transport,
            "mode": case.mode,
        }
        for case in cases
    ]
    checks_by_case = {
        f"{case.transport}:{case.mode}": {check.name: check for check in case.checks}
        for case in cases
    }
    observed = {check.name for case in cases for check in case.checks}
    ordered_names = [name for name in CHECK_ORDER if name in observed]
    ordered_names.extend(sorted(observed - set(ordered_names)))
    rows: list[dict[str, object]] = []
    for name in ordered_names:
        results: dict[str, str] = {}
        for column in columns:
            key = str(column["key"])
            check = checks_by_case[key].get(name)
            results[key] = "N/A" if check is None else check.status
        rows.append({"test": name, "results": results})
    return {"columns": columns, "rows": rows}


def _call_tool(
    client: AppServerClient,
    *,
    thread_id: str,
    tool: str,
    arguments: Mapping[str, object],
    meta: Mapping[str, object] | None = None,
) -> tuple[dict[str, object] | None, str]:
    params: dict[str, object] = {
        "threadId": thread_id,
        "server": TEST_SERVER_NAME,
        "tool": tool,
        "arguments": dict(arguments),
    }
    if meta is not None:
        params["_meta"] = dict(meta)
    return _response_result(client.request("mcpServer/tool/call", params))


def _exercise_app_server(
    case: CaseResult,
    client: AppServerClient,
    *,
    transport: str,
    mode: str,
    workspace: Path,
    enable_modern_feature: bool,
) -> None:
    initialize, initialize_detail = _response_result(
        client.request(
            "initialize",
            {
                "clientInfo": {
                    "name": "mcp-spec-compliance-runner",
                    "title": "MCP spec compliance runner",
                    "version": "1.0.0",
                },
                "capabilities": {
                    "experimentalApi": True,
                    "requestAttestation": False,
                    "mcpServerOpenaiFormElicitation": True,
                },
            },
        )
    )
    if not case.check(
        "app_server_initialize",
        initialize is not None,
        initialize_detail,
    ):
        return
    client.notify("initialized")

    if mode == MODERN_VERSION and enable_modern_feature:
        feature_result, feature_detail = _response_result(
            client.request(
                "experimentalFeature/enablement/set",
                {"enablement": {"mcp_2026_07_28": True}},
            )
        )
        enabled = (
            feature_result is not None
            and isinstance(feature_result.get("enablement"), dict)
            and feature_result["enablement"].get("mcp_2026_07_28") is True
        )
        if not case.check(
            "modern_feature_enablement",
            enabled,
            (
                "runtime feature mcp_2026_07_28 enabled"
                if enabled
                else _result_detail(feature_result, feature_detail)
            ),
        ):
            return

    inventory, inventory_detail = _response_result(
        client.request("mcpServerStatus/list", {})
    )
    if inventory is None:
        case.check("inventory", False, inventory_detail)
    else:
        valid, detail = _validate_inventory(inventory, mode=mode)
        case.check("inventory", valid, detail)

    thread_result, thread_detail = _response_result(
        client.request(
            "thread/start",
            {"cwd": str(workspace), "ephemeral": True},
        )
    )
    thread_id: str | None = None
    if thread_result is not None:
        thread_value = thread_result.get("thread")
        if isinstance(thread_value, dict) and isinstance(thread_value.get("id"), str):
            thread_id = str(thread_value["id"])
    if not case.check(
        "ephemeral_thread",
        thread_id is not None,
        "ephemeral thread created" if thread_id is not None else thread_detail,
    ):
        return
    assert thread_id is not None

    sentinel = f"codex-mcp-{transport}-{mode}"
    echo, echo_detail = _call_tool(
        client,
        thread_id=thread_id,
        tool="echo",
        arguments={"text": sentinel},
        meta={TEST_META_KEY: sentinel},
    )
    if echo is None:
        case.check("echo_tool", False, echo_detail)
    else:
        valid, detail = _validate_echo(echo, sentinel)
        case.check("echo_tool", valid, detail)

    resource, resource_detail = _response_result(
        client.request(
            "mcpServer/resource/read",
            {
                "threadId": thread_id,
                "server": TEST_SERVER_NAME,
                "uri": RESOURCE_URIS[1],
            },
        )
    )
    expected_resource_text = f"fixture contents for {RESOURCE_URIS[1]}"
    contents = resource.get("contents") if resource is not None else None
    resource_ok = (
        isinstance(contents, list)
        and len(contents) == 1
        and isinstance(contents[0], dict)
        and contents[0].get("text") == expected_resource_text
    )
    case.check(
        "unicode_resource_read",
        resource_ok,
        (
            "read the Unicode resource"
            if resource_ok
            else _result_detail(resource, resource_detail)
        ),
    )

    if mode != MODERN_VERSION:
        return

    metadata, metadata_detail = _call_tool(
        client,
        thread_id=thread_id,
        tool="client_metadata",
        arguments={},
        meta={TEST_META_KEY: sentinel},
    )
    structured = metadata.get("structuredContent") if metadata is not None else None
    metadata_ok = (
        isinstance(structured, dict)
        and structured.get("io.modelcontextprotocol/protocolVersion") == MODERN_VERSION
        and isinstance(
            structured.get("io.modelcontextprotocol/clientCapabilities"),
            dict,
        )
        and structured.get(TEST_META_KEY) == sentinel
    )
    case.check(
        "per_request_metadata",
        metadata_ok,
        (
            "required and caller metadata were preserved"
            if metadata_ok
            else _result_detail(metadata, metadata_detail)
        ),
    )

    progress, progress_detail = _call_tool(
        client,
        thread_id=thread_id,
        tool="progress",
        arguments={},
        meta={
            "progressToken": "fixture-progress",
            "io.modelcontextprotocol/logLevel": "info",
        },
    )
    progress_content = progress.get("content") if progress is not None else None
    progress_ok = isinstance(progress_content, list) and bool(progress_content)
    case.check(
        "request_scoped_notifications",
        progress_ok,
        (
            "progress and log notifications were consumed before the result"
            if progress_ok
            else _result_detail(progress, progress_detail)
        ),
    )

    elicitations_before = len(client.elicitation_requests)
    mrtr, mrtr_detail = _call_tool(
        client,
        thread_id=thread_id,
        tool="request_input",
        arguments={},
    )
    mrtr_structured = mrtr.get("structuredContent") if mrtr is not None else None
    new_elicitations = client.elicitation_requests[elicitations_before:]
    mrtr_ok = (
        isinstance(mrtr_structured, dict)
        and mrtr_structured.get("confirmation") == "confirmed"
        and any(
            item.get("serverName") == TEST_SERVER_NAME and item.get("mode") == "form"
            for item in new_elicitations
        )
    )
    case.check(
        "multi_round_trip_request",
        mrtr_ok,
        (
            "elicitation was surfaced, answered, retried, and completed"
            if mrtr_ok
            else _result_detail(mrtr, mrtr_detail)
        ),
    )

    if transport == "http":
        arguments = {
            "region": "us-west1",
            "attempt": 3,
            "enabled": True,
            "greeting": "Hello, 世界",
        }
        mirrored, mirrored_detail = _call_tool(
            client,
            thread_id=thread_id,
            tool="header_echo",
            arguments=arguments,
        )
        mirrored_structured = (
            mirrored.get("structuredContent") if mirrored is not None else None
        )
        mirrored_ok = mirrored_structured == arguments
        case.check(
            "http_header_mirroring",
            mirrored_ok,
            (
                "method, name, scalar, and Base64 headers matched"
                if mirrored_ok
                else _result_detail(mirrored, mirrored_detail)
            ),
        )


def _run_case(
    codex_binary: Path,
    server_script: Path,
    *,
    transport: str,
    mode: str,
    case_home: Path,
    timeout_seconds: float,
    enable_modern_feature: bool,
) -> CaseResult:
    started_at = time.monotonic()
    case = CaseResult(transport=transport, mode=mode)
    case_home.mkdir(parents=True)
    workspace = case_home / "workspace"
    workspace.mkdir()
    env = _isolated_environment(case_home)
    client: AppServerClient | None = None
    registered = False

    http_context = (
        _running_http_fixture(mode) if transport == "http" else nullcontext(None)
    )
    try:
        with http_context as http_url:
            add = _run_command(
                _registration_command(
                    codex_binary,
                    server_script,
                    transport=transport,
                    mode=mode,
                    http_url=http_url,
                ),
                env=env,
                cwd=workspace,
                timeout_seconds=timeout_seconds,
            )
            registered = case.check(
                "mcp_add",
                add.returncode == 0,
                _command_detail(add),
            )
            if registered:
                get = _run_command(
                    [
                        str(codex_binary),
                        "mcp",
                        "get",
                        TEST_SERVER_NAME,
                        "--json",
                    ],
                    env=env,
                    cwd=workspace,
                    timeout_seconds=timeout_seconds,
                )
                config: dict[str, object] | None = None
                if get.returncode == 0:
                    try:
                        decoded = json.loads(get.stdout)
                        if isinstance(decoded, dict):
                            config = decoded
                    except json.JSONDecodeError:
                        pass
                if config is None:
                    case.check("mcp_get", False, _command_detail(get))
                else:
                    valid, detail = _validate_registration(
                        config,
                        transport=transport,
                        mode=mode,
                        http_url=http_url,
                    )
                    case.check("mcp_get", valid, detail)

                client = AppServerClient(
                    codex_binary,
                    env=env,
                    cwd=workspace,
                    timeout_seconds=timeout_seconds,
                )
                try:
                    _exercise_app_server(
                        case,
                        client,
                        transport=transport,
                        mode=mode,
                        workspace=workspace,
                        enable_modern_feature=enable_modern_feature,
                    )
                except (AppServerError, OSError) as exc:
                    case.check("app_server_protocol", False, str(exc))
                finally:
                    client.close()
                    diagnostic_text = client.diagnostic_text()
                    if diagnostic_text and any(
                        not check.success for check in case.checks
                    ):
                        case.diagnostics = diagnostic_text
    except OSError as exc:
        case.check("case_runtime", False, str(exc))
    finally:
        if client is not None and client.process.poll() is None:
            client.close()
        if registered:
            remove = _run_command(
                [str(codex_binary), "mcp", "remove", TEST_SERVER_NAME],
                env=env,
                cwd=workspace,
                timeout_seconds=timeout_seconds,
            )
            case.check(
                "mcp_remove",
                remove.returncode == 0,
                _command_detail(remove),
            )
            listed = _run_command(
                [str(codex_binary), "mcp", "list", "--json"],
                env=env,
                cwd=workspace,
                timeout_seconds=timeout_seconds,
            )
            empty = False
            if listed.returncode == 0:
                try:
                    empty = json.loads(listed.stdout) == []
                except json.JSONDecodeError:
                    pass
            case.check(
                "isolated_config_cleanup",
                empty,
                "no MCP servers remained" if empty else _command_detail(listed),
            )
        case.finish(started_at)
    return case


def _official_check_result(
    result: OfficialScenarioResult,
    *,
    index: int,
    duplicate: bool,
) -> CheckResult:
    check = result.checks[index]
    if check.status == "SUCCESS":
        status = "PASS"
        success = True
    elif check.status in {"SKIPPED", "INFO"}:
        status = "SKIP"
        success = True
    else:
        status = "FAIL"
        success = False
    suffix = f"{check.name}#{index + 1}" if duplicate else check.name
    name = f"official/{result.scenario}/{suffix}"
    detail = check.error_message or check.description
    return CheckResult(
        name=name,
        success=success,
        detail=detail,
        status=status,
        source="official",
        scenario=result.scenario,
        check_id=check.check_id,
        category="auth" if result.scenario.startswith("auth/") else "non-auth",
    )


def _run_official_case(
    codex_binary: Path,
    adapter_script: Path,
    *,
    conformance_command: Sequence[str],
    mode: str,
    scenarios: Sequence[str],
    case_home: Path,
    timeout_seconds: float,
    enable_modern_feature: bool,
    require_automatic_auth: bool = False,
) -> CaseResult:
    started_at = time.monotonic()
    case = CaseResult(transport="official-http", mode=mode)
    case_home.mkdir(parents=True)
    env = _isolated_environment(case_home)
    env["CODEX_CONFORMANCE_TIMEOUT"] = str(timeout_seconds)
    env["CODEX_CONFORMANCE_ENABLE_MODERN_FEATURE"] = (
        "1" if enable_modern_feature else "0"
    )
    env["CODEX_CONFORMANCE_REQUIRE_AUTOMATIC_AUTH"] = (
        "1" if require_automatic_auth else "0"
    )
    results = run_official_mode(
        conformance_command=conformance_command,
        adapter_script=adapter_script,
        codex_binary=codex_binary,
        mode=mode,
        scenarios=scenarios,
        output_dir=case_home / "official-results",
        timeout_seconds=timeout_seconds,
        base_env=env,
    )

    diagnostics: list[str] = []
    for result in results:
        counts: dict[str, int] = {}
        for check in result.checks:
            counts[check.name] = counts.get(check.name, 0) + 1
        for index, check in enumerate(result.checks):
            case.checks.append(
                _official_check_result(
                    result,
                    index=index,
                    duplicate=counts[check.name] > 1,
                )
            )

        case.checks.append(
            CheckResult(
                name=f"harness/{result.scenario}/codex-adapter",
                success=result.adapter_success,
                detail=result.adapter_detail,
                status="PASS" if result.adapter_success else "FAIL",
                source="harness",
                scenario=result.scenario,
                category=(
                    "auth" if result.scenario.startswith("auth/") else "non-auth"
                ),
            )
        )
        if (
            result.adapter_success
            and not result.success
            and not any(
                check.status in {"FAILURE", "WARNING"} for check in result.checks
            )
        ):
            case.checks.append(
                CheckResult(
                    name=f"harness/{result.scenario}/official-runner",
                    success=False,
                    detail=result.runner_detail or "official runner failed",
                    status="FAIL",
                    source="harness",
                    scenario=result.scenario,
                    category=(
                        "auth" if result.scenario.startswith("auth/") else "non-auth"
                    ),
                )
            )
        if not result.success and result.runner_detail:
            diagnostics.append(f"{result.scenario}:\n{result.runner_detail}")

    if not results:
        case.checks.append(
            CheckResult(
                name="harness/official-scenario-selection",
                success=False,
                detail=f"no official scenarios selected for {mode}",
                status="FAIL",
                source="harness",
            )
        )
    if diagnostics:
        case.diagnostics = "\n\n".join(diagnostics)[-16_000:]
    case.finish(started_at)
    return case


def run_compliance(
    codex_binary: Path,
    *,
    server_script: Path,
    adapter_script: Path,
    conformance_command: Sequence[str],
    official_scenarios: Sequence[str] | None,
    modes: Sequence[str],
    transports: Sequence[str],
    timeout_seconds: float,
    artifact_parent: Path | None,
    keep_artifacts: bool,
    enable_modern_feature: bool = True,
    include_auth: bool = True,
    require_automatic_auth: bool = False,
) -> tuple[dict[str, object], Path | None]:
    started_at = datetime.now(timezone.utc)
    run_root = Path(
        tempfile.mkdtemp(
            prefix="codex-mcp-compliance-",
            dir=artifact_parent,
        )
    )
    version_env = _isolated_environment(run_root)
    version = _run_command(
        [str(codex_binary), "--version"],
        env=version_env,
        cwd=run_root,
        timeout_seconds=timeout_seconds,
    )

    cases: list[CaseResult] = []
    selected_scenarios: dict[str, tuple[str, ...]] = {
        mode: scenarios_for_mode(
            mode,
            official_scenarios,
            include_auth=include_auth,
        )
        for mode in modes
    }

    # Transport order is intentional: supplemental local stdio first, then the
    # official suite's loopback HTTP server.
    for transport in transports:
        for mode in modes:
            if transport == "stdio":
                case_home = run_root / f"stdio-{mode}"
                cases.append(
                    _run_case(
                        codex_binary,
                        server_script,
                        transport="stdio",
                        mode=mode,
                        case_home=case_home,
                        timeout_seconds=timeout_seconds,
                        enable_modern_feature=enable_modern_feature,
                    )
                )
            else:
                if not selected_scenarios[mode]:
                    continue
                case_home = run_root / f"official-http-{mode}"
                cases.append(
                    _run_official_case(
                        codex_binary,
                        adapter_script,
                        conformance_command=conformance_command,
                        mode=mode,
                        scenarios=selected_scenarios[mode],
                        case_home=case_home,
                        timeout_seconds=timeout_seconds,
                        enable_modern_feature=enable_modern_feature,
                        require_automatic_auth=require_automatic_auth,
                    )
                )

    passed = sum(case.success for case in cases)
    finished_at = datetime.now(timezone.utc)
    report: dict[str, object] = {
        "schemaVersion": REPORT_SCHEMA_VERSION,
        "success": version.returncode == 0 and passed == len(cases),
        "startedAt": started_at.isoformat(),
        "finishedAt": finished_at.isoformat(),
        "codexBinary": str(codex_binary),
        "codexVersion": version.stdout or None,
        "modernFeatureEnablement": enable_modern_feature,
        "automaticAuthRequired": require_automatic_auth,
        "officialConformance": {
            "repository": OFFICIAL_CONFORMANCE_REPOSITORY,
            "gitRef": OFFICIAL_CONFORMANCE_GIT_REF,
            "scope": (
                "all versioned client scenarios"
                if include_auth
                else "versioned non-auth client scenarios"
            ),
            "authenticationIncluded": include_auth,
            "scenarios": selected_scenarios,
        },
        "versionCheck": {
            "success": version.returncode == 0,
            "detail": _command_detail(version),
        },
        "summary": {
            "passed": passed,
            "failed": len(cases) - passed,
            "total": len(cases),
        },
        "modeSummaries": _summarize_modes(cases),
        "testMatrix": _build_test_matrix(cases),
        "cases": [asdict(case) for case in cases],
        "artifacts": str(run_root) if keep_artifacts else None,
    }

    retained: Path | None = run_root if keep_artifacts else None
    if not keep_artifacts:
        shutil.rmtree(run_root, ignore_errors=True)
    return report, retained


def _regression_check_identity(
    mode: str,
    transport: str,
    check: Mapping[str, object],
) -> _RegressionCheckIdentity | None:
    source = check.get("source")
    scenario = check.get("scenario")
    name = check.get("name")
    check_id = check.get("check_id")
    if source not in {"official", "harness", "supplemental"}:
        return None
    if not isinstance(name, str) or not name:
        return None
    if scenario is not None and (not isinstance(scenario, str) or not scenario):
        return None
    if check_id is not None and (not isinstance(check_id, str) or not check_id):
        return None

    # The upstream runner numbers repeated request observations by arrival
    # order. Neither that number nor the number of passing retries is a stable
    # protocol assertion, so prefer the upstream check ID when it exists.
    stable_name = name
    stem, separator, suffix = name.rpartition("#")
    if separator and stem and suffix.isdecimal():
        stable_name = stem
    return _RegressionCheckIdentity(
        mode=mode,
        transport=transport,
        source=source,
        scenario=scenario if isinstance(scenario, str) else "",
        check_id=check_id if isinstance(check_id, str) else stable_name,
    )


def _required_regression_checks(
    report: Mapping[str, object],
    *,
    label: str,
    errors: list[str],
) -> dict[_RegressionCheckIdentity, set[str]]:
    if report.get("schemaVersion") != REPORT_SCHEMA_VERSION:
        errors.append(f"{label} does not use report schema {REPORT_SCHEMA_VERSION}")

    version_check = report.get("versionCheck")
    if not isinstance(version_check, dict) or version_check.get("success") is not True:
        errors.append(f"{label} does not contain a successful Codex version check")
    if report.get("modernFeatureEnablement") is not True:
        errors.append(f"{label} did not enable the modern MCP feature")

    official = report.get("officialConformance")
    selected: Mapping[str, object] = {}
    if not isinstance(official, dict):
        errors.append(f"{label} does not contain official conformance metadata")
    else:
        if official.get("repository") != OFFICIAL_CONFORMANCE_REPOSITORY:
            errors.append(f"{label} uses a different upstream conformance repository")
        if official.get("gitRef") != OFFICIAL_CONFORMANCE_GIT_REF:
            errors.append(
                f"{label} uses a different pinned upstream conformance revision"
            )
        if official.get("authenticationIncluded") is not True:
            errors.append(f"{label} does not include the required OAuth scenarios")
        scenarios = official.get("scenarios")
        if isinstance(scenarios, dict):
            selected = scenarios
        else:
            errors.append(f"{label} does not contain a versioned scenario catalog")

    baseline_kind = report.get("baselineKind")
    if baseline_kind is not None:
        if baseline_kind != COMPACT_REGRESSION_BASELINE_KIND:
            errors.append(f"{label} uses an unsupported compact baseline format")
            return {}
        if report.get("requiredModes") != list(REQUIRED_REGRESSION_MODES):
            errors.append(
                f"{label} does not require the shipping, intermediate, and modern MCP versions"
            )
        transports = report.get("transports")
        if (
            not isinstance(transports, list)
            or len(transports) != 2
            or set(transports) != {"stdio", "official-http"}
        ):
            errors.append(f"{label} does not require both stdio and official HTTP")

        raw_identities = report.get("checks")
        if not isinstance(raw_identities, dict):
            errors.append(f"{label} does not contain compact baseline check identities")
            return {}

        identities: dict[_RegressionCheckIdentity, set[str]] = {}
        for bucket, status in (("passing", "PASS"), ("failing", "FAIL")):
            records = raw_identities.get(bucket)
            if not isinstance(records, list):
                errors.append(f"{label} does not contain {bucket} baseline identities")
                continue
            seen: set[_RegressionCheckIdentity] = set()
            for record in records:
                if not isinstance(record, dict):
                    errors.append(
                        f"{label} contains a malformed {bucket} baseline identity"
                    )
                    continue
                mode = record.get("mode")
                transport = record.get("transport")
                source = record.get("source")
                scenario = record.get("scenario")
                check_id = record.get("check_id")
                if (
                    mode not in REQUIRED_REGRESSION_MODES
                    or transport not in {"stdio", "official-http"}
                    or source not in {"official", "harness", "supplemental"}
                    or not isinstance(scenario, str)
                    or not isinstance(check_id, str)
                    or not check_id
                ):
                    errors.append(
                        f"{label} contains a malformed {bucket} baseline identity"
                    )
                    continue
                identity = _RegressionCheckIdentity(
                    mode, transport, source, scenario, check_id
                )
                if identity in seen:
                    errors.append(
                        f"{label} contains a duplicate {bucket} baseline identity"
                    )
                    continue
                seen.add(identity)
                identities.setdefault(identity, set()).add(status)

        for mode in REQUIRED_REGRESSION_MODES:
            expected_scenarios = scenarios_for_mode(mode, include_auth=True)
            actual_scenarios = selected.get(mode)
            if (
                not isinstance(actual_scenarios, (list, tuple))
                or tuple(actual_scenarios) != expected_scenarios
            ):
                errors.append(
                    f"{label} does not run the complete authenticated {mode} scenario catalog"
                )
            for transport in ("stdio", "official-http"):
                if not any(
                    identity.mode == mode and identity.transport == transport
                    for identity in identities
                ):
                    errors.append(
                        f"{label} is missing the required {transport} case for {mode}"
                    )
            observed = {
                identity.scenario
                for identity in identities
                if identity.mode == mode
                and identity.transport == "official-http"
                and identity.source in {"official", "harness"}
            }
            missing = sorted(set(expected_scenarios) - observed)
            unexpected = sorted(observed - set(expected_scenarios))
            if missing:
                errors.append(
                    f"{label} did not observe {mode} scenarios: {', '.join(missing)}"
                )
            if unexpected:
                errors.append(
                    f"{label} observed unexpected {mode} scenarios: {', '.join(unexpected)}"
                )
        return identities

    case_list = report.get("cases")
    cases: dict[tuple[str, str], Mapping[str, object]] = {}
    if not isinstance(case_list, list):
        errors.append(f"{label} does not contain transport cases")
        case_list = []
    for case in case_list:
        if not isinstance(case, dict):
            errors.append(f"{label} contains a malformed transport case")
            continue
        mode = case.get("mode")
        transport = case.get("transport")
        if mode not in REQUIRED_REGRESSION_MODES:
            continue
        if not isinstance(mode, str) or not isinstance(transport, str):
            errors.append(f"{label} contains a malformed required transport case")
            continue
        key = (mode, transport)
        if key in cases:
            errors.append(f"{label} contains a duplicate {transport} case for {mode}")
            continue
        cases[key] = case

    identities: dict[_RegressionCheckIdentity, set[str]] = {}
    for mode in REQUIRED_REGRESSION_MODES:
        expected_scenarios = scenarios_for_mode(mode, include_auth=True)
        actual_scenarios = selected.get(mode)
        if (
            not isinstance(actual_scenarios, (list, tuple))
            or tuple(actual_scenarios) != expected_scenarios
        ):
            errors.append(
                f"{label} does not run the complete authenticated {mode} scenario catalog"
            )

        for transport in ("stdio", "official-http"):
            case = cases.get((mode, transport))
            if case is None:
                errors.append(
                    f"{label} is missing the required {transport} case for {mode}"
                )
                continue
            raw_checks = case.get("checks")
            if not isinstance(raw_checks, list) or not raw_checks:
                errors.append(f"{label} has no {transport} checks for {mode}")
                continue

            observed_scenarios: set[str] = set()
            for check in raw_checks:
                if not isinstance(check, dict):
                    errors.append(
                        f"{label} contains a malformed {transport} check for {mode}"
                    )
                    continue
                identity = _regression_check_identity(mode, transport, check)
                status = check.get("status")
                success = check.get("success")
                if (
                    identity is None
                    or status not in {"PASS", "FAIL", "SKIP"}
                    or not isinstance(success, bool)
                    or success != (status != "FAIL")
                ):
                    errors.append(
                        f"{label} contains a malformed {transport} check for {mode}"
                    )
                    continue
                identities.setdefault(identity, set()).add(status)
                if transport == "official-http" and identity.source in {
                    "official",
                    "harness",
                }:
                    observed_scenarios.add(identity.scenario)

            if transport == "official-http":
                expected = set(expected_scenarios)
                missing = sorted(expected - observed_scenarios)
                unexpected = sorted(observed_scenarios - expected)
                if missing:
                    errors.append(
                        f"{label} did not observe {mode} scenarios: {', '.join(missing)}"
                    )
                if unexpected:
                    errors.append(
                        f"{label} observed unexpected {mode} scenarios: {', '.join(unexpected)}"
                    )

    return identities


def _compact_regression_baseline(report: Mapping[str, object]) -> dict[str, object]:
    errors: list[str] = []
    identities = _required_regression_checks(report, label="baseline", errors=errors)
    automatic_auth = report.get("automaticAuthRequired")
    if not isinstance(automatic_auth, bool):
        errors.append("baseline does not record its production OAuth policy")
    if any(
        identity.mode == SHIPPING_LEGACY_VERSION and "FAIL" in statuses
        for identity, statuses in identities.items()
    ):
        errors.append("baseline contains a shipping MCP regression")
    if errors:
        raise ValueError("; ".join(errors))

    official = report.get("officialConformance")
    assert isinstance(official, dict)
    scenarios = official.get("scenarios")
    assert isinstance(scenarios, dict)
    return {
        "baselineKind": COMPACT_REGRESSION_BASELINE_KIND,
        "schemaVersion": REPORT_SCHEMA_VERSION,
        "requiredModes": list(REQUIRED_REGRESSION_MODES),
        "transports": ["stdio", "official-http"],
        "versionCheck": {"success": True},
        "modernFeatureEnablement": True,
        "automaticAuthRequired": automatic_auth,
        "officialConformance": {
            "repository": OFFICIAL_CONFORMANCE_REPOSITORY,
            "gitRef": OFFICIAL_CONFORMANCE_GIT_REF,
            "authenticationIncluded": True,
            "scenarios": {mode: scenarios[mode] for mode in REQUIRED_REGRESSION_MODES},
        },
        "checks": {
            "passing": [
                asdict(identity)
                for identity, statuses in sorted(identities.items())
                if "PASS" in statuses
            ],
            "failing": [
                asdict(identity)
                for identity, statuses in sorted(identities.items())
                if "FAIL" in statuses
            ],
        },
    }


def _write_json_file(path: Path, value: Mapping[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _write_compact_regression_baseline(
    report: Mapping[str, object],
    path: Path,
) -> None:
    _write_json_file(path, _compact_regression_baseline(report))


def _evaluate_regression_gate(
    report: Mapping[str, object],
    baseline_report: Mapping[str, object],
    *,
    baseline_path: Path | None = None,
) -> dict[str, object]:
    errors: list[str] = []
    baseline_checks = _required_regression_checks(
        baseline_report, label="baseline", errors=errors
    )
    candidate_checks = _required_regression_checks(
        report, label="candidate", errors=errors
    )

    baseline_auth = baseline_report.get("automaticAuthRequired")
    candidate_auth = report.get("automaticAuthRequired")
    if not isinstance(baseline_auth, bool) or not isinstance(candidate_auth, bool):
        errors.append(
            "baseline and candidate must record their production OAuth policy"
        )
    elif baseline_auth != candidate_auth:
        errors.append("baseline and candidate use different production OAuth policies")

    new_failures: list[dict[str, object]] = []
    missing_checks: list[dict[str, object]] = []
    known_failures: list[dict[str, object]] = []
    fixed_checks: list[dict[str, object]] = []

    for identity, statuses in sorted(candidate_checks.items()):
        baseline_statuses = baseline_checks.get(identity, set())
        if "FAIL" not in statuses:
            continue
        if identity.mode == SHIPPING_LEGACY_VERSION:
            errors.append(f"shipping MCP check failed: {identity.check_id}")
        if "FAIL" in baseline_statuses:
            known_failures.append(asdict(identity))
        else:
            new_failures.append(asdict(identity))

    for identity, baseline_statuses in sorted(baseline_checks.items()):
        candidate_statuses = candidate_checks.get(identity, set())
        if identity.mode == SHIPPING_LEGACY_VERSION and "FAIL" in baseline_statuses:
            errors.append(
                f"baseline shipping MCP check is not passing: {identity.check_id}"
            )
        if "PASS" in baseline_statuses and "PASS" not in candidate_statuses:
            missing_checks.append(asdict(identity))
        elif (
            "FAIL" in baseline_statuses
            and "FAIL" not in candidate_statuses
            and "PASS" not in candidate_statuses
        ):
            missing_checks.append(asdict(identity))
        elif "FAIL" in baseline_statuses and "FAIL" not in candidate_statuses:
            fixed_checks.append(asdict(identity))

    return {
        "success": not errors and not new_failures and not missing_checks,
        "requiredModes": list(REQUIRED_REGRESSION_MODES),
        "baselineReport": str(baseline_path) if baseline_path is not None else None,
        "configurationErrors": errors,
        "newFailures": new_failures,
        "missingChecks": missing_checks,
        "knownFailures": known_failures,
        "fixedChecks": fixed_checks,
    }


def _format_ratio(value: Mapping[str, object]) -> str:
    passed = value.get("passed", 0)
    total = value.get("total", 0)
    if total == 0:
        return "0/0 (N/A)"
    percentage = value.get("percentage", 0)
    return f"{passed}/{total} ({percentage:g}%)"


def _print_table(headers: Sequence[str], rows: Sequence[Sequence[str]]) -> None:
    widths = [
        max(len(headers[index]), *(len(row[index]) for row in rows))
        for index in range(len(headers))
    ]

    def print_row(row: Sequence[str], separator: str = " | ") -> None:
        print(
            separator.join(
                value.ljust(widths[index]) for index, value in enumerate(row)
            )
        )

    print_row(headers)
    print("-+-".join("-" * width for width in widths))
    for row in rows:
        print_row(row)


def _print_human_report(report: Mapping[str, object]) -> None:
    success = report.get("success") is True
    print(f"Codex MCP compliance: {'PASS' if success else 'FAIL'}")
    print(f"Binary: {report.get('codexBinary')}")
    print(f"Version: {report.get('codexVersion') or 'unknown'}")
    failure_details: list[tuple[str, str, str]] = []
    cases = report.get("cases")
    if isinstance(cases, list):
        for case in cases:
            if not isinstance(case, dict):
                continue
            label = f"{case.get('transport')} / {case.get('mode')}"
            print(f"  {'PASS' if case.get('success') else 'FAIL'} {label}")
            checks = case.get("checks")
            if not isinstance(checks, list):
                continue
            for check in checks:
                if isinstance(check, dict) and check.get("success") is not True:
                    failure_details.append(
                        (
                            label,
                            str(check.get("name")),
                            str(check.get("detail")),
                        )
                    )

    mode_summaries = report.get("modeSummaries")
    if isinstance(mode_summaries, list) and mode_summaries:
        print("\nPass rates by protocol mode")
        summary_rows: list[list[str]] = []
        for item in mode_summaries:
            if not isinstance(item, dict):
                continue
            checks = item.get("checks")
            official_checks = item.get("officialChecks")
            official_non_auth_checks = item.get("officialNonAuthChecks")
            official_auth_checks = item.get("officialAuthChecks")
            official_non_auth_scenarios = item.get("officialNonAuthScenarios")
            official_auth_scenarios = item.get("officialAuthScenarios")
            harness_checks = item.get("harnessChecks")
            supplemental_checks = item.get("supplementalChecks")
            transport_cases = item.get("transportCases")
            if not all(
                isinstance(value, dict)
                for value in (
                    checks,
                    official_checks,
                    official_non_auth_checks,
                    official_auth_checks,
                    official_non_auth_scenarios,
                    official_auth_scenarios,
                    harness_checks,
                    supplemental_checks,
                    transport_cases,
                )
            ):
                continue
            summary_rows.append(
                [
                    str(item.get("mode")),
                    _format_ratio(official_non_auth_scenarios),
                    _format_ratio(official_auth_scenarios),
                    _format_ratio(official_checks),
                    _format_ratio(harness_checks),
                    _format_ratio(supplemental_checks),
                    _format_ratio(checks),
                    _format_ratio(transport_cases),
                ]
            )
        _print_table(
            [
                "Mode",
                "Non-auth scenarios",
                "Auth scenarios",
                "Official assertions",
                "Harness",
                "Supplemental",
                "All checks",
                "Transport cases",
            ],
            summary_rows,
        )

    test_matrix = report.get("testMatrix")
    if isinstance(test_matrix, dict):
        columns = test_matrix.get("columns")
        rows = test_matrix.get("rows")
        if isinstance(columns, list) and isinstance(rows, list) and columns:
            matrix_headers = ["Test"]
            column_keys: list[str] = []
            for column in columns:
                if not isinstance(column, dict):
                    continue
                key = str(column.get("key"))
                column_keys.append(key)
                matrix_headers.append(f"{column.get('transport')} {column.get('mode')}")
            matrix_rows: list[list[str]] = []
            for row in rows:
                if not isinstance(row, dict) or not isinstance(
                    row.get("results"), dict
                ):
                    continue
                results = row["results"]
                matrix_rows.append(
                    [
                        str(row.get("test")),
                        *(str(results.get(key, "N/A")) for key in column_keys),
                    ]
                )
            print("\nPer-test results")
            _print_table(matrix_headers, matrix_rows)

    if failure_details:
        print("\nFailure details")
        for label, name, detail in failure_details:
            print(f"  {label} / {name}: {detail}")

    summary = report.get("summary")
    if isinstance(summary, dict):
        print(f"Summary: {summary.get('passed')}/{summary.get('total')} cases passed")
    gate = report.get("regressionGate")
    if isinstance(gate, dict):
        passed = gate.get("success") is True
        known = gate.get("knownFailures")
        known_count = len(known) if isinstance(known, list) else 0
        print(
            f"Regression gate: {'PASS' if passed else 'FAIL'} "
            f"({known_count} acknowledged baseline failures)"
        )
        for category in ("configurationErrors", "newFailures", "missingChecks"):
            values = gate.get(category)
            if isinstance(values, list):
                for value in values:
                    print(f"  {category}: {value}")
    artifacts = report.get("artifacts")
    if isinstance(artifacts, str):
        print(f"Artifacts: {artifacts}")


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run the full versioned official MCP client conformance suite against a "
            "supplied Codex binary, plus the supplemental local stdio suite."
        )
    )
    parser.add_argument("codex_binary", type=Path)
    parser.add_argument(
        "--mode",
        choices=("all", SHIPPING_LEGACY_VERSION, LEGACY_VERSION, MODERN_VERSION),
        default="all",
        help="Protocol era to test (default: both).",
    )
    parser.add_argument(
        "--transport",
        choices=("all", "stdio", "http"),
        default="all",
        help=(
            "Transport to test: supplemental stdio, official localhost HTTP, "
            "or both (default: both)."
        ),
    )
    parser.add_argument(
        "--server-script",
        type=Path,
        default=Path(__file__).resolve().with_name("server.py"),
        help="Path to the MCP fixture server.py.",
    )
    parser.add_argument(
        "--adapter-script",
        type=Path,
        default=Path(__file__).resolve().with_name("codex_conformance_adapter.py"),
        help="Path to the Codex adapter used by the official client suite.",
    )
    parser.add_argument(
        "--conformance-cli",
        type=Path,
        help=(
            "Use an existing official conformance executable. By default the "
            "runner bootstraps the pinned upstream Git commit with npx."
        ),
    )
    parser.add_argument(
        "--official-scenario",
        action="append",
        help=(
            "Run only this official versioned scenario. Repeat to select more "
            "(default: every applicable scenario for the selected mode)."
        ),
    )
    parser.add_argument(
        "--auth",
        action=argparse.BooleanOptionalAction,
        default=True,
        help=(
            "Include official OAuth authorization scenarios in HTTP coverage "
            "(default: enabled). Use --no-auth for a faster non-auth slice."
        ),
    )
    parser.add_argument(
        "--require-automatic-auth",
        action="store_true",
        help=(
            "Require Codex to recover from OAuth scope escalation and "
            "authorization-server migration without harness-injected re-login; "
            "also exercise production client-metadata selection."
        ),
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=45.0,
        help="Per-command and per-request timeout in seconds.",
    )
    parser.add_argument(
        "--report",
        type=Path,
        help="Also write the complete JSON report to this path.",
    )
    parser.add_argument(
        "--baseline-report",
        type=Path,
        help=(
            "Require the complete shipping, intermediate, and modern authenticated "
            "HTTP and stdio matrices to preserve every passing check from this report. "
            "Known baseline failures remain visible and new failures fail the gate."
        ),
    )
    parser.add_argument(
        "--write-baseline",
        type=Path,
        help="Write a compact, deterministic regression baseline from the completed run.",
    )
    parser.add_argument(
        "--extract-baseline",
        type=Path,
        help=(
            "Write a compact, deterministic baseline from --baseline-report "
            "and exit without rerunning conformance."
        ),
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Print the report as JSON instead of the human summary.",
    )
    parser.add_argument(
        "--artifact-parent",
        type=Path,
        help="Parent directory for isolated temporary Codex homes.",
    )
    parser.add_argument(
        "--keep-artifacts",
        action="store_true",
        help="Keep isolated Codex homes and include their path in the report.",
    )
    parser.add_argument(
        "--enable-modern-feature",
        action=argparse.BooleanOptionalAction,
        default=True,
        help=(
            "Configure mcp_2026_07_28 before MCP startup and verify it through "
            "the app-server runtime setter for modern cases (default: enabled)."
        ),
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    codex_binary = args.codex_binary.expanduser().resolve()
    server_script = args.server_script.expanduser().resolve()
    adapter_script = args.adapter_script.expanduser().resolve()
    modes = (
        (SHIPPING_LEGACY_VERSION, LEGACY_VERSION, MODERN_VERSION)
        if args.mode == "all"
        else (args.mode,)
    )
    transports = ("stdio", "http") if args.transport == "all" else (args.transport,)
    if not codex_binary.is_file() or not os.access(codex_binary, os.X_OK):
        print(f"error: Codex binary is not executable: {codex_binary}", file=sys.stderr)
        return 2
    if "stdio" in transports and not server_script.is_file():
        print(f"error: fixture server does not exist: {server_script}", file=sys.stderr)
        return 2
    if "http" in transports and not adapter_script.is_file():
        print(
            f"error: official conformance adapter does not exist: {adapter_script}",
            file=sys.stderr,
        )
        return 2
    if args.timeout <= 0:
        print("error: --timeout must be positive", file=sys.stderr)
        return 2
    if args.extract_baseline is not None and args.baseline_report is None:
        print("error: --extract-baseline requires --baseline-report", file=sys.stderr)
        return 2
    if args.extract_baseline is not None and args.write_baseline is not None:
        print(
            "error: --extract-baseline cannot be combined with --write-baseline",
            file=sys.stderr,
        )
        return 2

    baseline_report: dict[str, object] | None = None
    baseline_path: Path | None = None
    if args.baseline_report is not None:
        baseline_path = args.baseline_report.expanduser().resolve()
        try:
            loaded_baseline = json.loads(baseline_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            print(
                f"error: cannot read baseline report {baseline_path}: {exc}",
                file=sys.stderr,
            )
            return 2
        if not isinstance(loaded_baseline, dict):
            print(
                f"error: baseline report must be a JSON object: {baseline_path}",
                file=sys.stderr,
            )
            return 2
        baseline_report = loaded_baseline

    if args.extract_baseline is not None:
        assert baseline_report is not None
        output_path = args.extract_baseline.expanduser().resolve()
        try:
            _write_compact_regression_baseline(baseline_report, output_path)
        except (OSError, ValueError) as exc:
            print(
                f"error: cannot extract regression baseline {output_path}: {exc}",
                file=sys.stderr,
            )
            return 2
        print(f"Regression baseline: {output_path}")
        return 0

    conformance_command: list[str]
    if args.conformance_cli is None:
        conformance_command = default_conformance_command()
    else:
        conformance_cli = args.conformance_cli.expanduser().resolve()
        if not conformance_cli.is_file():
            print(
                f"error: official conformance CLI does not exist: {conformance_cli}",
                file=sys.stderr,
            )
            return 2
        if os.access(conformance_cli, os.X_OK):
            conformance_command = [str(conformance_cli)]
        elif conformance_cli.suffix in {".js", ".mjs"} and shutil.which("node"):
            conformance_command = [str(shutil.which("node")), str(conformance_cli)]
        else:
            print(
                "error: --conformance-cli must be executable (or a JavaScript "
                f"file runnable with node): {conformance_cli}",
                file=sys.stderr,
            )
            return 2

    try:
        for mode in modes:
            scenarios_for_mode(
                mode,
                args.official_scenario,
                include_auth=args.auth,
            )
    except ValueError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    artifact_parent: Path | None = None
    if args.artifact_parent is not None:
        artifact_parent = args.artifact_parent.expanduser().resolve()
        artifact_parent.mkdir(parents=True, exist_ok=True)

    report, _ = run_compliance(
        codex_binary,
        server_script=server_script,
        adapter_script=adapter_script,
        conformance_command=conformance_command,
        official_scenarios=args.official_scenario,
        modes=modes,
        transports=transports,
        timeout_seconds=args.timeout,
        artifact_parent=artifact_parent,
        keep_artifacts=args.keep_artifacts,
        enable_modern_feature=args.enable_modern_feature,
        include_auth=args.auth,
        require_automatic_auth=args.require_automatic_auth,
    )
    if baseline_report is not None:
        report["regressionGate"] = _evaluate_regression_gate(
            report,
            baseline_report,
            baseline_path=baseline_path,
        )
    if args.write_baseline is not None:
        output_path = args.write_baseline.expanduser().resolve()
        try:
            _write_compact_regression_baseline(report, output_path)
        except (OSError, ValueError) as exc:
            print(
                f"error: cannot write regression baseline {output_path}: {exc}",
                file=sys.stderr,
            )
            return 2

    if args.report is not None:
        report_path = args.report.expanduser().resolve()
        _write_json_file(report_path, report)
    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    else:
        _print_human_report(report)
        if args.report is not None:
            print(f"JSON report: {args.report.expanduser().resolve()}")
    if baseline_report is not None:
        gate = report.get("regressionGate")
        return 0 if isinstance(gate, dict) and gate.get("success") is True else 1
    return 0 if report["success"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
