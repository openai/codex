#!/usr/bin/env python3
"""Run reviewer-derived MCP regressions against a real Codex app-server."""

import argparse
import json
import os
import shutil
import sys
import tempfile
import threading
import time
from contextlib import contextmanager, nullcontext
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterator, Mapping, Sequence

_MODULE_DIR = Path(__file__).resolve().parent
if str(_MODULE_DIR) not in sys.path:
    sys.path.insert(0, str(_MODULE_DIR))

from run_codex_compliance import (  # noqa: E402 - direct scripts must first add their sibling directory.
    LEGACY_VERSION,
    MISMATCHED_DISCOVERY_ID_PROFILE,
    MODERN_VERSION,
    NULL_DISCOVERY_ID_PROFILE,
    REPEATED_CURSOR_PROFILE,
    REVIEW_EXACT_INTEGER,
    REVIEW_MRTR_INPUT_REQUEST_COUNT,
    REVIEW_PROFILE,
    SHIPPING_LEGACY_VERSION,
    SSE_COMMENT_FLOOD_PROFILE,
    SSE_CR_COMMENTS_PROFILE,
    TEST_SERVER_NAME,
    AppServerClient,
    AppServerError,
    CaseResult,
    ProtocolServer,
    _call_tool,
    _command_detail,
    _isolated_environment,
    _response_result,
    _run_command,
    make_http_server,
)
from server import (  # noqa: E402 - direct scripts must first add their sibling directory.
    CATALOG_MAX_PROFILE,
    CATALOG_OVER_LIMIT_PROFILE,
    MAX_CATALOG_ITEMS,
)

REVIEW_MODES = (SHIPPING_LEGACY_VERSION, LEGACY_VERSION, MODERN_VERSION)
REVIEW_REPORT_SCHEMA_VERSION = 1
REVIEW_BASELINE_KIND = "mcp-review-regression-baseline-v1"
REVIEWER = "codex-mcp-regression-suite"
LEGACY_ENVIRONMENT_SENTINEL = "review-legacy-protocol-environment"
MRTR_REQUEST_LIMIT = 64
CATALOG_BOUNDARY_PROFILES = (CATALOG_MAX_PROFILE, CATALOG_OVER_LIMIT_PROFILE)
CATALOG_BOUNDARY_TRANSPORTS = ("stdio", "http")
CATALOG_LIMIT_ERROR = (
    f"tools/list exceeded the catalog limit of {MAX_CATALOG_ITEMS} items"
)


@dataclass(frozen=True, order=True)
class _ReviewCheckIdentity:
    mode: str
    transport: str
    check_id: str


def _required_review_cases() -> set[tuple[str, str]]:
    cases = {(mode, f"review-stdio:{REVIEW_PROFILE}") for mode in REVIEW_MODES}
    cases.update(
        (MODERN_VERSION, f"review-http:{profile}")
        for profile in (
            REVIEW_PROFILE,
            REPEATED_CURSOR_PROFILE,
            MISMATCHED_DISCOVERY_ID_PROFILE,
            NULL_DISCOVERY_ID_PROFILE,
            SSE_CR_COMMENTS_PROFILE,
            SSE_COMMENT_FLOOD_PROFILE,
        )
    )
    cases.update(
        (mode, f"review-{transport}:{profile}")
        for mode in REVIEW_MODES
        for transport in CATALOG_BOUNDARY_TRANSPORTS
        for profile in CATALOG_BOUNDARY_PROFILES
    )
    return cases


@contextmanager
def _running_review_http_fixture(mode: str, profile: str) -> Iterator[str]:
    server = make_http_server(
        ProtocolServer(mode, profile=profile),
        "127.0.0.1",
        0,
        log_requests=False,
    )
    thread = threading.Thread(
        target=server.serve_forever,
        name=f"mcp-review-{profile}-{mode}",
        daemon=True,
    )
    thread.start()
    try:
        _, port = server.server_address
        yield f"http://127.0.0.1:{port}/mcp"
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=3)


def _review_registration_command(
    codex_binary: Path,
    server_script: Path,
    *,
    transport: str,
    mode: str,
    profile: str,
    http_url: str | None,
) -> list[str]:
    command = [str(codex_binary), "mcp", "add", TEST_SERVER_NAME]
    if transport == "stdio":
        protocol_environment = (
            MODERN_VERSION if mode == MODERN_VERSION else LEGACY_ENVIRONMENT_SENTINEL
        )
        command.extend(
            [
                "--env",
                f"CODEX_MCP_PROTOCOL_VERSION={protocol_environment}",
                "--",
                sys.executable,
                str(server_script),
                "--mode",
                mode,
                "--transport",
                "stdio",
                "--profile",
                profile,
            ]
        )
    else:
        if http_url is None:
            raise ValueError("review HTTP transport requires a fixture URL")
        command.extend(["--url", http_url])
    return command


def _review_inventory_entry(result: Mapping[str, object]) -> dict[str, object] | None:
    entries = result.get("data")
    if not isinstance(entries, list):
        return None
    for entry in entries:
        if isinstance(entry, dict) and entry.get("name") == TEST_SERVER_NAME:
            return entry
    return None


def _tool_input_schema(
    entry: Mapping[str, object],
    tool_name: str,
) -> Mapping[str, object] | None:
    tools = entry.get("tools")
    if not isinstance(tools, dict):
        return None
    tool = tools.get(tool_name)
    if not isinstance(tool, dict):
        return None
    schema = tool.get("inputSchema")
    if not isinstance(schema, dict):
        schema = tool.get("input_schema")
    return schema if isinstance(schema, dict) else None


def _exact_integer_property(schema: Mapping[str, object] | None) -> bool:
    if schema is None:
        return False
    properties = schema.get("properties")
    if not isinstance(properties, dict):
        return False
    value = properties.get("value")
    return (
        isinstance(value, dict)
        and value.get("type") == "integer"
        and isinstance(value.get("minimum"), int)
        and not isinstance(value.get("minimum"), bool)
        and value.get("minimum") == REVIEW_EXACT_INTEGER
        and isinstance(value.get("default"), int)
        and not isinstance(value.get("default"), bool)
        and value.get("default") == REVIEW_EXACT_INTEGER
    )


def _elicitation_schema(request: Mapping[str, object]) -> Mapping[str, object] | None:
    for key in ("requestedSchema", "requested_schema", "schema"):
        schema = request.get(key)
        if isinstance(schema, dict):
            return schema
    for key in ("request", "elicitation", "params"):
        nested = request.get(key)
        if isinstance(nested, dict):
            found = _elicitation_schema(nested)
            if found is not None:
                return found
    return None


def _review_elicitation_content(params: Mapping[str, object]) -> Mapping[str, object]:
    schema = _elicitation_schema(params)
    if _exact_integer_property(schema):
        return {"value": REVIEW_EXACT_INTEGER}
    return {"value": "review-confirmed", "confirmation": "confirmed"}


def _mrtr_budget_is_bounded(
    request_count: int,
    *,
    elapsed_seconds: float,
    timeout_seconds: float,
) -> bool:
    return (
        0 <= request_count <= MRTR_REQUEST_LIMIT
        and elapsed_seconds <= timeout_seconds + 1
    )


def _initialize_review_client(
    case: CaseResult,
    client: AppServerClient,
    *,
    mode: str,
) -> bool:
    response, detail = _response_result(
        client.request(
            "initialize",
            {
                "clientInfo": {
                    "name": "mcp-review-regression-runner",
                    "title": "MCP review regression runner",
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
    if not case.check("review/app-server-initialize", response is not None, detail):
        return False
    client.notify("initialized")
    if mode != MODERN_VERSION:
        return True

    feature, feature_detail = _response_result(
        client.request(
            "experimentalFeature/enablement/set",
            {"enablement": {"mcp_2026_07_28": True}},
        )
    )
    enabled = (
        feature is not None
        and isinstance(feature.get("enablement"), dict)
        and feature["enablement"].get("mcp_2026_07_28") is True
    )
    return case.check(
        "review/modern-feature-enablement",
        enabled,
        "enabled the production modern MCP feature" if enabled else feature_detail,
    )


def _review_thread_id(
    case: CaseResult,
    client: AppServerClient,
    workspace: Path,
) -> str | None:
    response, detail = _response_result(
        client.request("thread/start", {"cwd": str(workspace), "ephemeral": True})
    )
    thread = response.get("thread") if response is not None else None
    thread_id = thread.get("id") if isinstance(thread, dict) else None
    if not case.check(
        "review/ephemeral-thread",
        isinstance(thread_id, str),
        "created isolated review thread" if isinstance(thread_id, str) else detail,
    ):
        return None
    return str(thread_id)


def _run_normal_review_checks(
    case: CaseResult,
    client: AppServerClient,
    *,
    mode: str,
    workspace: Path,
) -> None:
    inventory, inventory_detail = _response_result(
        client.request("mcpServerStatus/list", {"detail": "full"})
    )
    entry = _review_inventory_entry(inventory) if inventory is not None else None
    if not case.check(
        "review/server-discovery",
        entry is not None,
        "discovered the adversarial review fixture"
        if entry is not None
        else inventory_detail,
    ):
        return
    assert entry is not None

    tools = entry.get("tools")
    if not isinstance(tools, dict):
        tools = {}
    if mode == MODERN_VERSION:
        required_page_two = {
            "review_integer_elicitation",
            "review_large_integer",
            "review_mrtr_cap",
            "review_protocol_env",
        }
        missing = sorted(required_page_two - set(tools))
        case.check(
            "review/modern-paginated-tools-page-two",
            not missing,
            "all protected second-page tools reached the app-server inventory"
            if not missing
            else "second-page tools missing: " + ", ".join(missing),
        )

    schema = _tool_input_schema(entry, "review_large_integer")
    exact_schema = _exact_integer_property(schema)
    case.check(
        "review/exact-large-integer-tool-schema",
        exact_schema,
        "preserved integer minimum and default 9007199254740993"
        if exact_schema
        else "the app-server rounded or omitted the 2^53+1 tool schema",
    )

    thread_id = _review_thread_id(case, client, workspace)
    if thread_id is None:
        return

    integer, integer_detail = _call_tool(
        client,
        thread_id=thread_id,
        tool="review_large_integer",
        arguments={"value": REVIEW_EXACT_INTEGER},
    )
    content = integer.get("structuredContent") if integer is not None else None
    exact_value = (
        isinstance(content, dict)
        and isinstance(content.get("value"), int)
        and not isinstance(content.get("value"), bool)
        and content.get("value") == REVIEW_EXACT_INTEGER
    )
    case.check(
        "review/exact-large-integer-tool-round-trip",
        exact_value,
        "round-tripped 9007199254740993 without floating-point conversion"
        if exact_value
        else integer_detail,
    )

    if mode != MODERN_VERSION:
        environment, environment_detail = _call_tool(
            client,
            thread_id=thread_id,
            tool="review_protocol_env",
            arguments={},
        )
        observed = (
            environment.get("structuredContent") if environment is not None else None
        )
        preserved = (
            isinstance(observed, dict)
            and observed.get("value") == LEGACY_ENVIRONMENT_SENTINEL
        )
        case.check(
            "review/legacy-reserved-stdio-environment-preserved",
            preserved,
            "forwarded the explicitly configured legacy protocol environment unchanged"
            if preserved
            else environment_detail,
        )
        return

    before = len(client.elicitation_requests)
    elicitation, elicitation_detail = _call_tool(
        client,
        thread_id=thread_id,
        tool="review_integer_elicitation",
        arguments={},
    )
    requests = client.elicitation_requests[before:]
    exact_elicitation = any(
        _exact_integer_property(_elicitation_schema(request)) for request in requests
    )
    case.check(
        "review/exact-large-integer-elicitation-schema",
        exact_elicitation,
        "app-server preserved 9007199254740993 in the actual elicitation schema"
        if exact_elicitation
        else "the production elicitation schema rounded or omitted the 2^53+1 integer",
    )
    elicitation_content = (
        elicitation.get("structuredContent") if elicitation is not None else None
    )
    completed = (
        isinstance(elicitation_content, dict)
        and elicitation_content.get("value") == REVIEW_EXACT_INTEGER
    )
    case.check(
        "review/exact-large-integer-elicitation-round-trip",
        completed,
        "completed elicitation using the exact 2^53+1 integer"
        if completed
        else elicitation_detail,
    )

    mrtr_before = len(client.elicitation_requests)
    started = time.monotonic()
    mrtr_result = None
    try:
        mrtr_result, mrtr_detail = _call_tool(
            client,
            thread_id=thread_id,
            tool="review_mrtr_cap",
            arguments={},
        )
    except AppServerError as exc:
        mrtr_detail = str(exc)
    elapsed = time.monotonic() - started
    observed = len(client.elicitation_requests) - mrtr_before
    bounded = mrtr_result is None and _mrtr_budget_is_bounded(
        observed,
        elapsed_seconds=elapsed,
        timeout_seconds=client.timeout_seconds,
    )
    case.check(
        "review/mrtr-input-requests-bounded-to-64",
        bounded,
        (
            f"rejected {REVIEW_MRTR_INPUT_REQUEST_COUNT} simultaneous input requests; "
            f"observed {observed} elicitations in {elapsed:.2f}s"
            if bounded
            else (
                f"observed {observed} elicitation requests for a "
                f"{REVIEW_MRTR_INPUT_REQUEST_COUNT}-request response in "
                f"{elapsed:.2f}s; {mrtr_detail}"
            )
        ),
    )


def _run_malformed_discovery_check(
    case: CaseResult,
    client: AppServerClient,
    *,
    profile: str,
) -> None:
    name = (
        "review/reject-null-discovery-response-id"
        if profile == NULL_DISCOVERY_ID_PROFILE
        else "review/reject-mismatched-discovery-response-id"
    )
    try:
        inventory, detail = _response_result(
            client.request("mcpServerStatus/list", {"detail": "full"})
        )
    except AppServerError as exc:
        case.check(name, True, f"rejected malformed discovery response: {exc}")
        return
    entry = _review_inventory_entry(inventory) if inventory is not None else None
    tools = entry.get("tools") if entry is not None else None
    rejected = (
        inventory is None or entry is None or not isinstance(tools, dict) or not tools
    )
    case.check(
        name,
        rejected,
        "rejected the uncorrelated discovery response without silently downgrading"
        if rejected
        else "accepted an uncorrelated discovery response and exposed server tools",
    )


def _run_repeated_cursor_check(case: CaseResult, client: AppServerClient) -> None:
    started = time.monotonic()
    try:
        inventory, _ = _response_result(
            client.request("mcpServerStatus/list", {"detail": "full"})
        )
        entry = _review_inventory_entry(inventory) if inventory is not None else None
        tools = entry.get("tools") if entry is not None else None
        rejected = (
            inventory is None
            or entry is None
            or not isinstance(tools, dict)
            or not tools
        )
        detail = (
            "rejected the repeated pagination cursor within the startup deadline"
            if rejected
            else "accepted or exposed a catalog with a repeated pagination cursor"
        )
    except AppServerError as exc:
        rejected = True
        detail = f"terminated repeated-cursor pagination: {exc}"
    elapsed = time.monotonic() - started
    case.check(
        "review/repeated-pagination-cursor-bounded",
        rejected and elapsed <= client.timeout_seconds + 2,
        f"{detail}; elapsed={elapsed:.2f}s",
    )


def _run_catalog_boundary_checks(
    case: CaseResult,
    client: AppServerClient,
    *,
    profile: str,
    workspace: Path,
) -> None:
    if profile not in CATALOG_BOUNDARY_PROFILES:
        raise ValueError(f"unknown catalog boundary profile: {profile}")

    at_limit = profile == CATALOG_MAX_PROFILE
    check_prefix = (
        "review/catalog-at-limit" if at_limit else "review/catalog-over-limit"
    )
    expected_status = "ready" if at_limit else "failed"
    event_index = len(client.events)
    if _review_thread_id(case, client, workspace) is None:
        return

    try:
        event = client.wait_for_notification(
            "mcpServer/startupStatus/updated",
            predicate=lambda params: (
                params.get("name") == TEST_SERVER_NAME
                and params.get("status") in {"ready", "failed"}
            ),
            after_event_index=event_index,
        )
    except AppServerError as exc:
        case.check(
            f"{check_prefix}/startup-{expected_status}",
            False,
            f"did not observe the required {expected_status} startup: {exc}",
        )
        return

    params = event.get("params")
    case.check(
        f"{check_prefix}/startup-{expected_status}",
        isinstance(params, dict) and params.get("status") == expected_status,
        f"observed the expected {expected_status} MCP server startup"
        if isinstance(params, dict) and params.get("status") == expected_status
        else (
            "observed MCP server startup "
            f"{params.get('status')!r}; expected {expected_status!r}"
            if isinstance(params, dict)
            else "MCP server startup notification did not contain parameters"
        ),
    )
    if not isinstance(params, dict):
        return

    if not at_limit:
        error = params.get("error")
        case.check(
            f"{check_prefix}/exact-limit-error",
            isinstance(error, str) and CATALOG_LIMIT_ERROR in error,
            error
            if isinstance(error, str)
            else f"startup failure did not report {CATALOG_LIMIT_ERROR!r}",
        )

    inventory, detail = _response_result(
        client.request("mcpServerStatus/list", {"detail": "full"})
    )
    entry = _review_inventory_entry(inventory) if inventory is not None else None
    if not case.check(
        f"{check_prefix}/configured-server",
        entry is not None,
        "retained the configured catalog-boundary server"
        if entry is not None
        else detail,
    ):
        return
    assert entry is not None

    server_info = entry.get("serverInfo")
    expected_server_info = (
        isinstance(server_info, dict) if at_limit else server_info is None
    )
    case.check(
        f"{check_prefix}/server-info",
        expected_server_info,
        "exposed initialized server information"
        if at_limit and expected_server_info
        else "left rejected server information null"
        if not at_limit and expected_server_info
        else "catalog server information did not match its startup status",
    )

    tools = entry.get("tools")
    actual_count = len(tools) if isinstance(tools, dict) else None
    expected_count = MAX_CATALOG_ITEMS if at_limit else 0
    case.check(
        f"{check_prefix}/tool-count",
        isinstance(tools, dict) and actual_count == expected_count,
        f"discovered exactly {expected_count} tools"
        if actual_count == expected_count
        else f"expected {expected_count} discovered tools, observed {actual_count}",
    )


def _run_sse_check(
    case: CaseResult,
    client: AppServerClient,
    workspace: Path,
    *,
    profile: str,
) -> None:
    inventory, detail = _response_result(
        client.request("mcpServerStatus/list", {"detail": "full"})
    )
    entry = _review_inventory_entry(inventory) if inventory is not None else None
    if not case.check(
        "review/sse-cr-discovery",
        entry is not None,
        "discovered the CR-only SSE fixture" if entry is not None else detail,
    ):
        return
    thread_id = _review_thread_id(case, client, workspace)
    if thread_id is None:
        return
    result, detail = _call_tool(
        client,
        thread_id=thread_id,
        tool="progress",
        arguments={},
        meta={"progressToken": "review-cr-only-sse"},
    )
    content = result.get("content") if result is not None else None
    accepted = isinstance(content, list) and bool(content)
    flood = profile == SSE_COMMENT_FLOOD_PROFILE
    case.check(
        "review/sse-comment-flood-excluded-from-event-size-limit"
        if flood
        else "review/sse-carriage-return-and-comment-framing",
        accepted,
        "accepted more than 8 MiB of valid SSE comment keepalives"
        if accepted and flood
        else "accepted valid CR-only SSE lines and ignored comment keepalives"
        if accepted
        else detail,
    )


def _run_review_case(
    codex_binary: Path,
    server_script: Path,
    *,
    mode: str,
    profile: str,
    transport: str,
    case_home: Path,
    timeout_seconds: float,
) -> CaseResult:
    started = time.monotonic()
    case = CaseResult(transport=f"review-{transport}:{profile}", mode=mode)
    case_home.mkdir(parents=True, exist_ok=True)
    workspace = case_home / "workspace"
    workspace.mkdir(exist_ok=True)
    env = _isolated_environment(case_home)
    registered = False
    fixture = (
        _running_review_http_fixture(mode, profile)
        if transport == "http"
        else nullcontext(None)
    )

    try:
        if mode == MODERN_VERSION:
            feature = _run_command(
                [str(codex_binary), "features", "enable", "mcp_2026_07_28"],
                env=env,
                cwd=workspace,
                timeout_seconds=timeout_seconds,
            )
            if not case.check(
                "review/modern-feature-configuration",
                feature.returncode == 0,
                _command_detail(feature),
            ):
                return case

        with fixture as http_url:
            registration = _run_command(
                _review_registration_command(
                    codex_binary,
                    server_script,
                    transport=transport,
                    mode=mode,
                    profile=profile,
                    http_url=http_url,
                ),
                env=env,
                cwd=workspace,
                timeout_seconds=timeout_seconds,
            )
            registered = case.check(
                "review/mcp-registration",
                registration.returncode == 0,
                _command_detail(registration),
            )
            if not registered:
                return case

            with AppServerClient(
                codex_binary,
                env=env,
                cwd=workspace,
                timeout_seconds=timeout_seconds,
                elicitation_content=_review_elicitation_content,
            ) as client:
                if not _initialize_review_client(case, client, mode=mode):
                    return case
                if profile in (
                    MISMATCHED_DISCOVERY_ID_PROFILE,
                    NULL_DISCOVERY_ID_PROFILE,
                ):
                    _run_malformed_discovery_check(case, client, profile=profile)
                elif profile == REPEATED_CURSOR_PROFILE:
                    _run_repeated_cursor_check(case, client)
                elif profile in CATALOG_BOUNDARY_PROFILES:
                    _run_catalog_boundary_checks(
                        case, client, profile=profile, workspace=workspace
                    )
                elif profile in (SSE_CR_COMMENTS_PROFILE, SSE_COMMENT_FLOOD_PROFILE):
                    _run_sse_check(case, client, workspace, profile=profile)
                else:
                    _run_normal_review_checks(
                        case, client, mode=mode, workspace=workspace
                    )
    except (AppServerError, OSError, ValueError) as exc:
        case.check("review/client-runtime", False, str(exc))
    finally:
        if registered:
            removal = _run_command(
                [str(codex_binary), "mcp", "remove", TEST_SERVER_NAME],
                env=env,
                cwd=workspace,
                timeout_seconds=timeout_seconds,
            )
            case.check(
                "review/isolated-registration-cleanup",
                removal.returncode == 0,
                _command_detail(removal),
            )
        case.finish(started)
    return case


def run_review_regressions(
    codex_binary: Path,
    *,
    modes: Sequence[str] = REVIEW_MODES,
    server_script: Path | None = None,
    timeout_seconds: float = 8,
    artifact_parent: Path | None = None,
    keep_artifacts: bool = False,
) -> dict[str, object]:
    server_script = server_script or _MODULE_DIR / "server.py"
    root = Path(tempfile.mkdtemp(prefix="codex-mcp-review-", dir=artifact_parent))
    cases: list[CaseResult] = []
    try:
        for mode in modes:
            cases.append(
                _run_review_case(
                    codex_binary,
                    server_script,
                    mode=mode,
                    profile=REVIEW_PROFILE,
                    transport="stdio",
                    case_home=root / f"review-stdio-{mode}",
                    timeout_seconds=timeout_seconds,
                )
            )
            if mode != MODERN_VERSION:
                continue
            for profile in (
                REVIEW_PROFILE,
                REPEATED_CURSOR_PROFILE,
                MISMATCHED_DISCOVERY_ID_PROFILE,
                NULL_DISCOVERY_ID_PROFILE,
                SSE_CR_COMMENTS_PROFILE,
                SSE_COMMENT_FLOOD_PROFILE,
            ):
                cases.append(
                    _run_review_case(
                        codex_binary,
                        server_script,
                        mode=mode,
                        profile=profile,
                        transport="http",
                        case_home=root / f"review-http-{mode}-{profile}",
                        timeout_seconds=timeout_seconds,
                    )
                )

        for mode in modes:
            for transport in CATALOG_BOUNDARY_TRANSPORTS:
                for profile in CATALOG_BOUNDARY_PROFILES:
                    cases.append(
                        _run_review_case(
                            codex_binary,
                            server_script,
                            mode=mode,
                            profile=profile,
                            transport=transport,
                            case_home=root / f"review-{transport}-{mode}-{profile}",
                            timeout_seconds=timeout_seconds,
                        )
                    )

        checks = [check for case in cases for check in case.checks]
        failures = [check for check in checks if not check.success]
        return {
            "schemaVersion": REVIEW_REPORT_SCHEMA_VERSION,
            "success": bool(cases) and all(case.success for case in cases),
            "codexBinary": str(codex_binary),
            "reviewer": REVIEWER,
            "modes": list(modes),
            "summary": {
                "passed": len(checks) - len(failures),
                "failed": len(failures),
                "total": len(checks),
                "casesPassed": sum(case.success for case in cases),
                "casesTotal": len(cases),
            },
            "cases": [asdict(case) for case in cases],
            "artifacts": str(root) if keep_artifacts else None,
        }
    finally:
        if not keep_artifacts:
            shutil.rmtree(root, ignore_errors=True)


def _required_review_checks(
    report: Mapping[str, object],
    *,
    label: str,
    errors: list[str],
) -> dict[_ReviewCheckIdentity, bool]:
    if report.get("schemaVersion") != REVIEW_REPORT_SCHEMA_VERSION:
        errors.append(
            f"{label} does not use reviewer report schema "
            f"{REVIEW_REPORT_SCHEMA_VERSION}"
        )
    if report.get("reviewer") != REVIEWER:
        errors.append(f"{label} does not identify the production reviewer probes")

    compact = report.get("baselineKind") is not None
    modes = report.get("requiredModes" if compact else "modes")
    if modes != list(REVIEW_MODES):
        errors.append(f"{label} does not run all three required MCP protocol modes")

    expected_cases = _required_review_cases()
    observed_cases: set[tuple[str, str]] = set()
    identities: dict[_ReviewCheckIdentity, bool] = {}

    if compact:
        if report.get("baselineKind") != REVIEW_BASELINE_KIND:
            errors.append(f"{label} uses an unsupported reviewer baseline format")

        raw_cases = report.get("requiredCases")
        if not isinstance(raw_cases, list):
            errors.append(f"{label} does not contain required reviewer cases")
            raw_cases = []
        for case in raw_cases:
            if not isinstance(case, dict):
                errors.append(f"{label} contains a malformed reviewer case")
                continue
            mode = case.get("mode")
            transport = case.get("transport")
            if not isinstance(mode, str) or not isinstance(transport, str):
                errors.append(f"{label} contains a malformed reviewer case")
                continue
            identity = (mode, transport)
            if identity in observed_cases:
                errors.append(f"{label} contains a duplicate reviewer case")
                continue
            observed_cases.add(identity)

        raw_checks = report.get("checks")
        if not isinstance(raw_checks, dict):
            errors.append(f"{label} does not contain reviewer check identities")
            raw_checks = {}
        for bucket, success in (("passing", True), ("failing", False)):
            records = raw_checks.get(bucket)
            if not isinstance(records, list):
                errors.append(f"{label} does not contain {bucket} reviewer checks")
                continue
            for record in records:
                if not isinstance(record, dict):
                    errors.append(
                        f"{label} contains a malformed {bucket} reviewer check"
                    )
                    continue
                mode = record.get("mode")
                transport = record.get("transport")
                check_id = record.get("check_id")
                if (
                    not isinstance(mode, str)
                    or not isinstance(transport, str)
                    or not isinstance(check_id, str)
                    or not check_id.startswith("review/")
                    or (mode, transport) not in expected_cases
                ):
                    errors.append(
                        f"{label} contains a malformed {bucket} reviewer check"
                    )
                    continue
                identity = _ReviewCheckIdentity(mode, transport, check_id)
                if identity in identities:
                    errors.append(f"{label} contains a duplicate reviewer check")
                    continue
                identities[identity] = success
    else:
        raw_cases = report.get("cases")
        if not isinstance(raw_cases, list):
            errors.append(f"{label} does not contain production reviewer cases")
            raw_cases = []
        for case in raw_cases:
            if not isinstance(case, dict):
                errors.append(f"{label} contains a malformed reviewer case")
                continue
            mode = case.get("mode")
            transport = case.get("transport")
            if (
                not isinstance(mode, str)
                or not isinstance(transport, str)
                or (mode, transport) not in expected_cases
            ):
                errors.append(f"{label} contains an unexpected reviewer case")
                continue
            case_identity = (mode, transport)
            if case_identity in observed_cases:
                errors.append(f"{label} contains a duplicate reviewer case")
                continue
            observed_cases.add(case_identity)

            checks = case.get("checks")
            if not isinstance(checks, list) or not checks:
                errors.append(
                    f"{label} has no reviewer checks for {transport} / {mode}"
                )
                continue
            case_success = True
            for check in checks:
                if not isinstance(check, dict):
                    errors.append(f"{label} contains a malformed reviewer check")
                    continue
                check_id = check.get("name")
                success = check.get("success")
                if (
                    not isinstance(check_id, str)
                    or not check_id.startswith("review/")
                    or not isinstance(success, bool)
                ):
                    errors.append(f"{label} contains a malformed reviewer check")
                    continue
                identity = _ReviewCheckIdentity(mode, transport, check_id)
                if identity in identities:
                    errors.append(f"{label} contains a duplicate reviewer check")
                    continue
                identities[identity] = success
                case_success = case_success and success
            if case.get("success") is not case_success:
                errors.append(
                    f"{label} reports inconsistent reviewer case success "
                    f"for {transport} / {mode}"
                )

    missing_cases = sorted(expected_cases - observed_cases)
    unexpected_cases = sorted(observed_cases - expected_cases)
    for mode, transport in missing_cases:
        errors.append(f"{label} is missing required reviewer case {transport} / {mode}")
    for mode, transport in unexpected_cases:
        errors.append(f"{label} contains unexpected reviewer case {transport} / {mode}")

    summary = report.get("summary")
    passing = sum(identities.values())
    failed = len(identities) - passing
    cases_passed = sum(
        all(
            success
            for identity, success in identities.items()
            if (identity.mode, identity.transport) == case
        )
        and any((identity.mode, identity.transport) == case for identity in identities)
        for case in observed_cases
    )
    expected_summary = {
        "passed": passing,
        "failed": failed,
        "total": len(identities),
        "casesPassed": cases_passed,
        "casesTotal": len(observed_cases),
    }
    if not isinstance(summary, dict) or any(
        summary.get(name) != value for name, value in expected_summary.items()
    ):
        errors.append(f"{label} reports inconsistent reviewer check totals")
    if not compact and report.get("success") is not (
        bool(observed_cases) and failed == 0
    ):
        errors.append(f"{label} reports inconsistent overall reviewer success")
    return identities


def _compact_review_regression_baseline(
    report: Mapping[str, object],
) -> dict[str, object]:
    errors: list[str] = []
    checks = _required_review_checks(report, label="baseline", errors=errors)
    if errors:
        raise ValueError("; ".join(errors))

    summary = report.get("summary")
    assert isinstance(summary, dict)
    return {
        "baselineKind": REVIEW_BASELINE_KIND,
        "schemaVersion": REVIEW_REPORT_SCHEMA_VERSION,
        "reviewer": REVIEWER,
        "requiredModes": list(REVIEW_MODES),
        "requiredCases": [
            {"mode": mode, "transport": transport}
            for mode, transport in sorted(_required_review_cases())
        ],
        "summary": {
            key: summary[key]
            for key in ("passed", "failed", "total", "casesPassed", "casesTotal")
        },
        "checks": {
            "passing": [
                asdict(identity)
                for identity, success in sorted(checks.items())
                if success
            ],
            "failing": [
                asdict(identity)
                for identity, success in sorted(checks.items())
                if not success
            ],
        },
    }


def _evaluate_review_regression_gate(
    report: Mapping[str, object],
    baseline_report: Mapping[str, object],
    *,
    baseline_path: Path | None = None,
) -> dict[str, object]:
    errors: list[str] = []
    baseline_checks = _required_review_checks(
        baseline_report, label="baseline", errors=errors
    )
    candidate_checks = _required_review_checks(report, label="candidate", errors=errors)
    known_failures: list[dict[str, object]] = []
    new_failures: list[dict[str, object]] = []
    missing_checks: list[dict[str, object]] = []
    fixed_checks: list[dict[str, object]] = []

    for identity, success in sorted(candidate_checks.items()):
        if success:
            continue
        if baseline_checks.get(identity) is False:
            known_failures.append(asdict(identity))
        else:
            new_failures.append(asdict(identity))

    for identity, baseline_success in sorted(baseline_checks.items()):
        candidate_success = candidate_checks.get(identity)
        if candidate_success is None:
            missing_checks.append(asdict(identity))
        elif not baseline_success and candidate_success:
            fixed_checks.append(asdict(identity))

    return {
        "success": not errors and not new_failures and not missing_checks,
        "requiredModes": list(REVIEW_MODES),
        "baselineReport": str(baseline_path) if baseline_path is not None else None,
        "configurationErrors": errors,
        "knownFailures": known_failures,
        "newFailures": new_failures,
        "missingChecks": missing_checks,
        "fixedChecks": fixed_checks,
    }


def _write_review_json(path: Path, value: Mapping[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run reviewer-derived MCP client regression tests against Codex."
    )
    parser.add_argument("codex_binary", type=Path)
    parser.add_argument("--mode", choices=("all", *REVIEW_MODES), default="all")
    parser.add_argument("--timeout", type=float, default=8)
    parser.add_argument("--server-script", type=Path, default=_MODULE_DIR / "server.py")
    parser.add_argument("--report", type=Path)
    parser.add_argument(
        "--baseline-report",
        type=Path,
        help="Compare every production reviewer check with a reviewed baseline.",
    )
    parser.add_argument(
        "--write-baseline",
        type=Path,
        help="Write a compact reviewer baseline from the completed full matrix.",
    )
    parser.add_argument(
        "--extract-baseline",
        type=Path,
        help="Extract a compact reviewer baseline from --baseline-report and exit.",
    )
    parser.add_argument("--artifact-parent", type=Path)
    parser.add_argument("--keep-artifacts", action="store_true")
    parser.add_argument("--json", action="store_true")
    return parser.parse_args(argv)


def _print_report(report: Mapping[str, object]) -> None:
    print(
        "Codex MCP reviewer regressions: "
        + ("PASS" if report.get("success") is True else "FAIL")
    )
    print(f"Binary: {report.get('codexBinary')}")
    cases = report.get("cases")
    if isinstance(cases, list):
        for case in cases:
            if not isinstance(case, dict):
                continue
            print(f"\n{case.get('transport')} / {case.get('mode')}")
            for check in case.get("checks", []):
                if isinstance(check, dict):
                    status = "PASS" if check.get("success") else "FAIL"
                    print(f"  {status} {check.get('name')}: {check.get('detail')}")
    summary = report.get("summary")
    if isinstance(summary, dict):
        print(
            f"\nSummary: {summary.get('passed')}/{summary.get('total')} checks; "
            f"{summary.get('casesPassed')}/{summary.get('casesTotal')} cases"
        )
    gate = report.get("regressionGate")
    if isinstance(gate, dict):
        status = "PASS" if gate.get("success") is True else "FAIL"
        known = gate.get("knownFailures")
        new = gate.get("newFailures")
        missing = gate.get("missingChecks")
        fixed = gate.get("fixedChecks")
        print(
            f"Regression gate: {status}; "
            f"{len(known) if isinstance(known, list) else 0} known, "
            f"{len(new) if isinstance(new, list) else 0} new, "
            f"{len(missing) if isinstance(missing, list) else 0} missing, "
            f"{len(fixed) if isinstance(fixed, list) else 0} fixed"
        )
        errors = gate.get("configurationErrors")
        if isinstance(errors, list):
            for error in errors:
                print(f"  Configuration error: {error}")


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    codex_binary = args.codex_binary.expanduser().resolve()
    server_script = args.server_script.expanduser().resolve()
    if not codex_binary.is_file() or not os.access(codex_binary, os.X_OK):
        print(f"error: Codex binary is not executable: {codex_binary}", file=sys.stderr)
        return 2
    if not server_script.is_file():
        print(f"error: review fixture does not exist: {server_script}", file=sys.stderr)
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
        except (OSError, UnicodeError, json.JSONDecodeError) as exc:
            print(
                f"error: cannot read reviewer baseline {baseline_path}: {exc}",
                file=sys.stderr,
            )
            return 2
        if not isinstance(loaded_baseline, dict):
            print(
                f"error: reviewer baseline must be a JSON object: {baseline_path}",
                file=sys.stderr,
            )
            return 2
        baseline_report = loaded_baseline

    if args.extract_baseline is not None:
        assert baseline_report is not None
        output_path = args.extract_baseline.expanduser().resolve()
        try:
            _write_review_json(
                output_path, _compact_review_regression_baseline(baseline_report)
            )
        except (OSError, ValueError) as exc:
            print(f"error: cannot extract reviewer baseline: {exc}", file=sys.stderr)
            return 2
        print(f"Reviewer regression baseline: {output_path}")
        return 0

    parent = None
    if args.artifact_parent is not None:
        parent = args.artifact_parent.expanduser().resolve()
        parent.mkdir(parents=True, exist_ok=True)

    modes = REVIEW_MODES if args.mode == "all" else (args.mode,)
    report = run_review_regressions(
        codex_binary,
        modes=modes,
        server_script=server_script,
        timeout_seconds=args.timeout,
        artifact_parent=parent,
        keep_artifacts=args.keep_artifacts,
    )
    if baseline_report is not None:
        report["regressionGate"] = _evaluate_review_regression_gate(
            report,
            baseline_report,
            baseline_path=baseline_path,
        )
    if args.write_baseline is not None:
        output_path = args.write_baseline.expanduser().resolve()
        try:
            _write_review_json(output_path, _compact_review_regression_baseline(report))
        except (OSError, ValueError) as exc:
            print(f"error: cannot write reviewer baseline: {exc}", file=sys.stderr)
            return 2
    if args.report is not None:
        path = args.report.expanduser().resolve()
        _write_review_json(path, report)
    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    else:
        _print_report(report)
        if args.report is not None:
            print(f"JSON report: {args.report.expanduser().resolve()}")
        if args.write_baseline is not None:
            print(f"Reviewer regression baseline: {args.write_baseline.resolve()}")
    if baseline_report is not None:
        gate = report.get("regressionGate")
        return 0 if isinstance(gate, dict) and gate.get("success") is True else 1
    return 0 if report.get("success") is True else 1


if __name__ == "__main__":
    raise SystemExit(main())
