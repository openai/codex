import json
import sys
import time
from copy import deepcopy
from pathlib import Path
from typing import Callable, Mapping, Sequence

import pytest
from review_regressions import (
    CATALOG_BOUNDARY_PROFILES,
    CATALOG_BOUNDARY_TRANSPORTS,
    CATALOG_LIMIT_ERROR,
    CATALOG_MAX_PROFILE,
    CATALOG_OVER_LIMIT_PROFILE,
    LEGACY_ENVIRONMENT_SENTINEL,
    LEGACY_VERSION,
    MAX_CATALOG_ITEMS,
    MISMATCHED_DISCOVERY_ID_PROFILE,
    MODERN_VERSION,
    NULL_DISCOVERY_ID_PROFILE,
    REPEATED_CURSOR_PROFILE,
    REVIEW_EXACT_INTEGER,
    REVIEW_BASELINE_KIND,
    REVIEW_MODES,
    REVIEW_PROFILE,
    REVIEW_REPORT_SCHEMA_VERSION,
    REVIEWER,
    SHIPPING_LEGACY_VERSION,
    SSE_COMMENT_FLOOD_PROFILE,
    SSE_CR_COMMENTS_PROFILE,
    TEST_SERVER_NAME,
    AppServerError,
    CaseResult,
    _compact_review_regression_baseline,
    _elicitation_schema,
    _evaluate_review_regression_gate,
    _exact_integer_property,
    _mrtr_budget_is_bounded,
    _parse_args,
    _required_review_cases,
    _required_review_checks,
    _run_catalog_boundary_checks,
    _review_elicitation_content,
    _review_inventory_entry,
    _review_registration_command,
    _tool_input_schema,
    _write_review_json,
    main,
    run_review_regressions,
)


def _integer_schema(value: int = REVIEW_EXACT_INTEGER) -> dict[str, object]:
    return {
        "type": "object",
        "properties": {
            "value": {
                "type": "integer",
                "minimum": value,
                "maximum": 9_223_372_036_854_775_807,
                "default": value,
            }
        },
    }


class _CatalogBoundaryClient:
    def __init__(
        self,
        *,
        status: str | None,
        tool_count: int,
        server_info: dict[str, object] | None,
        error: str | None = None,
        notification_name: str = TEST_SERVER_NAME,
        preceding_statuses: Sequence[str] = (),
    ) -> None:
        self.events: list[dict[str, object]] = []
        self._status = status
        self._error = error
        self._notification_name = notification_name
        self._preceding_statuses = preceding_statuses
        self._entry: dict[str, object] = {
            "name": TEST_SERVER_NAME,
            "serverInfo": server_info,
            "tools": {
                f"catalog_boundary_{index:04d}": {} for index in range(tool_count)
            },
        }

    def request(
        self, method: str, params: Mapping[str, object] | None
    ) -> dict[str, object]:
        if method == "thread/start":
            if self._status is not None:
                for status in self._preceding_statuses:
                    self.events.append(
                        {
                            "method": "mcpServer/startupStatus/updated",
                            "params": {
                                "name": self._notification_name,
                                "status": status,
                            },
                        }
                    )
                notification_params: dict[str, object] = {
                    "name": self._notification_name,
                    "status": self._status,
                }
                if self._error is not None:
                    notification_params["error"] = self._error
                self.events.append(
                    {
                        "method": "mcpServer/startupStatus/updated",
                        "params": notification_params,
                    }
                )
            return {"result": {"thread": {"id": "catalog-boundary-thread"}}}
        if method == "mcpServerStatus/list":
            return {"result": {"data": [self._entry]}}
        raise AssertionError(f"unexpected app-server request: {method}")

    def wait_for_notification(
        self,
        method: str,
        *,
        predicate: Callable[[Mapping[str, object]], bool] | None = None,
        after_event_index: int = 0,
    ) -> dict[str, object]:
        for event in self.events[after_event_index:]:
            params = event.get("params")
            if (
                event.get("method") == method
                and isinstance(params, dict)
                and (predicate is None or predicate(params))
            ):
                return event
        raise AppServerError(f"timed out waiting for app-server notification {method}")


def test_review_matrix_includes_the_actual_shipping_legacy_protocol() -> None:
    assert REVIEW_MODES == ("2025-06-18", "2025-11-25", "2026-07-28")
    assert SHIPPING_LEGACY_VERSION == "2025-06-18"


def test_exact_integer_schema_does_not_accept_float_rounded_values() -> None:
    assert _exact_integer_property(_integer_schema())
    assert not _exact_integer_property(_integer_schema(REVIEW_EXACT_INTEGER - 1))

    rounded = _integer_schema()
    rounded["properties"]["value"]["minimum"] = float(REVIEW_EXACT_INTEGER)  # type: ignore[index]
    assert not _exact_integer_property(rounded)


def test_exact_integer_schema_does_not_accept_boolean_values() -> None:
    schema = _integer_schema()
    schema["properties"]["value"]["minimum"] = True  # type: ignore[index]

    assert not _exact_integer_property(schema)


@pytest.mark.parametrize("key", ["requestedSchema", "requested_schema", "schema"])
def test_extracts_the_actual_elicitation_requested_schema(key: str) -> None:
    schema = _integer_schema()

    assert _elicitation_schema({key: schema}) == schema
    assert _elicitation_schema({"params": {key: schema}}) == schema


def test_elicitation_response_preserves_the_exact_large_integer() -> None:
    response = _review_elicitation_content({"requestedSchema": _integer_schema()})

    assert response == {"value": 9_007_199_254_740_993}


@pytest.mark.parametrize(
    ("request_count", "elapsed_seconds", "expected"),
    [
        (0, 0.1, True),
        (64, 1.0, True),
        (65, 1.0, False),
        (64, 10.1, False),
    ],
)
def test_mrtr_budget_requires_both_request_and_time_limits(
    request_count: int,
    elapsed_seconds: float,
    expected: bool,
) -> None:
    assert (
        _mrtr_budget_is_bounded(
            request_count,
            elapsed_seconds=elapsed_seconds,
            timeout_seconds=8,
        )
        is expected
    )


def test_review_inventory_requires_the_registered_server() -> None:
    entry = {"name": "mcp_spec_fixture", "tools": {}}

    assert _review_inventory_entry({"data": [entry]}) == entry
    assert _review_inventory_entry({"data": [{"name": "another-server"}]}) is None
    assert _review_inventory_entry({"data": "invalid"}) is None


@pytest.mark.parametrize("schema_key", ["inputSchema", "input_schema"])
def test_review_inventory_recognizes_both_tool_schema_spellings(
    schema_key: str,
) -> None:
    schema = _integer_schema()
    entry = {"tools": {"review_large_integer": {schema_key: schema}}}

    assert _tool_input_schema(entry, "review_large_integer") == schema


def test_shipping_legacy_registration_preserves_the_reserved_protocol_environment() -> (
    None
):
    command = _review_registration_command(
        Path("/opt/codex"),
        Path("/src/server.py"),
        transport="stdio",
        mode=SHIPPING_LEGACY_VERSION,
        profile=REVIEW_PROFILE,
        http_url=None,
    )

    assert command == [
        "/opt/codex",
        "mcp",
        "add",
        "mcp_spec_fixture",
        "--env",
        f"CODEX_MCP_PROTOCOL_VERSION={LEGACY_ENVIRONMENT_SENTINEL}",
        "--",
        sys.executable,
        "/src/server.py",
        "--mode",
        SHIPPING_LEGACY_VERSION,
        "--transport",
        "stdio",
        "--profile",
        REVIEW_PROFILE,
    ]


def test_modern_registration_explicitly_opts_into_the_modern_protocol() -> None:
    command = _review_registration_command(
        Path("/opt/codex"),
        Path("/src/server.py"),
        transport="stdio",
        mode=MODERN_VERSION,
        profile=REVIEW_PROFILE,
        http_url=None,
    )

    assert f"CODEX_MCP_PROTOCOL_VERSION={MODERN_VERSION}" in command


def test_http_review_registration_requires_a_real_fixture_url() -> None:
    with pytest.raises(ValueError, match="fixture URL"):
        _review_registration_command(
            Path("/opt/codex"),
            Path("/src/server.py"),
            transport="http",
            mode=MODERN_VERSION,
            profile=REVIEW_PROFILE,
            http_url=None,
        )


def test_review_cli_supports_the_real_shipping_protocol_and_json_report() -> None:
    args = _parse_args(
        [
            "/opt/codex",
            "--mode",
            SHIPPING_LEGACY_VERSION,
            "--report",
            "/tmp/mcp-review-regressions.json",
        ]
    )

    assert args.mode == "2025-06-18"
    assert args.report == Path("/tmp/mcp-review-regressions.json")


def test_review_cli_reports_a_missing_codex_binary(
    capsys: pytest.CaptureFixture[str],
) -> None:
    assert main(["/path/to/nonexistent-review-codex"]) == 2
    assert "not executable" in capsys.readouterr().err


@pytest.mark.parametrize("mode", REVIEW_MODES)
def test_catalog_at_limit_requires_a_ready_server_and_all_tools(
    mode: str, tmp_path: Path
) -> None:
    case = CaseResult(transport="review-stdio:catalog-max", mode=mode)
    client = _CatalogBoundaryClient(
        status="ready",
        tool_count=MAX_CATALOG_ITEMS,
        server_info={"name": "catalog-boundary-server"},
    )

    _run_catalog_boundary_checks(
        case,
        client,  # type: ignore[arg-type] - exercise the real app-server client contract.
        profile=CATALOG_MAX_PROFILE,
        workspace=tmp_path,
    )
    case.finish(time.monotonic())

    assert case.success
    assert [check.name for check in case.checks] == [
        "review/ephemeral-thread",
        "review/catalog-at-limit/startup-ready",
        "review/catalog-at-limit/configured-server",
        "review/catalog-at-limit/server-info",
        "review/catalog-at-limit/tool-count",
    ]


@pytest.mark.parametrize("mode", REVIEW_MODES)
def test_clean_catalog_over_limit_rejection_is_an_expected_pass(
    mode: str, tmp_path: Path
) -> None:
    case = CaseResult(transport="review-http:catalog-over-limit", mode=mode)
    client = _CatalogBoundaryClient(
        status="failed",
        tool_count=0,
        server_info=None,
        error=f"MCP startup failed: {CATALOG_LIMIT_ERROR}",
    )

    _run_catalog_boundary_checks(
        case,
        client,  # type: ignore[arg-type] - exercise the real app-server client contract.
        profile=CATALOG_OVER_LIMIT_PROFILE,
        workspace=tmp_path,
    )
    case.finish(time.monotonic())

    assert case.success
    assert [check.name for check in case.checks] == [
        "review/ephemeral-thread",
        "review/catalog-over-limit/startup-failed",
        "review/catalog-over-limit/exact-limit-error",
        "review/catalog-over-limit/configured-server",
        "review/catalog-over-limit/server-info",
        "review/catalog-over-limit/tool-count",
    ]


@pytest.mark.parametrize(
    ("profile", "terminal_status", "tool_count"),
    [
        (CATALOG_MAX_PROFILE, "ready", MAX_CATALOG_ITEMS),
        (CATALOG_OVER_LIMIT_PROFILE, "failed", 0),
    ],
)
def test_catalog_boundary_ignores_starting_until_the_real_terminal_notification(
    profile: str,
    terminal_status: str,
    tool_count: int,
    tmp_path: Path,
) -> None:
    at_limit = profile == CATALOG_MAX_PROFILE
    case = CaseResult(transport=f"review-http:{profile}", mode=MODERN_VERSION)
    client = _CatalogBoundaryClient(
        status=terminal_status,
        tool_count=tool_count,
        server_info={"name": "catalog-boundary-server"} if at_limit else None,
        error=f"MCP startup failed: {CATALOG_LIMIT_ERROR}" if not at_limit else None,
        preceding_statuses=("starting",),
    )

    _run_catalog_boundary_checks(
        case,
        client,  # type: ignore[arg-type] - exercise the real app-server client contract.
        profile=profile,
        workspace=tmp_path,
    )
    case.finish(time.monotonic())

    assert case.success
    assert client.events[0]["params"]["status"] == "starting"  # type: ignore[index]
    assert client.events[1]["params"]["status"] == terminal_status  # type: ignore[index]


def test_catalog_over_limit_does_not_accept_an_unrelated_startup_failure(
    tmp_path: Path,
) -> None:
    case = CaseResult(transport="review-stdio:catalog-over-limit", mode=MODERN_VERSION)
    client = _CatalogBoundaryClient(
        status="failed",
        tool_count=0,
        server_info=None,
        error="MCP startup failed: connection unexpectedly closed",
    )

    _run_catalog_boundary_checks(
        case,
        client,  # type: ignore[arg-type] - exercise the real app-server client contract.
        profile=CATALOG_OVER_LIMIT_PROFILE,
        workspace=tmp_path,
    )
    case.finish(time.monotonic())

    assert not case.success
    assert [check.name for check in case.checks if not check.success] == [
        "review/catalog-over-limit/exact-limit-error"
    ]


@pytest.mark.parametrize("mode", REVIEW_MODES)
@pytest.mark.parametrize("transport", CATALOG_BOUNDARY_TRANSPORTS)
def test_catalog_over_limit_records_all_checks_when_startup_incorrectly_succeeds(
    mode: str, transport: str, tmp_path: Path
) -> None:
    case = CaseResult(transport=f"review-{transport}:catalog-over-limit", mode=mode)
    client = _CatalogBoundaryClient(
        status="ready",
        tool_count=MAX_CATALOG_ITEMS + 1,
        server_info={"name": "catalog-boundary-server"},
    )

    _run_catalog_boundary_checks(
        case,
        client,  # type: ignore[arg-type] - exercise the real app-server client contract.
        profile=CATALOG_OVER_LIMIT_PROFILE,
        workspace=tmp_path,
    )
    case.finish(time.monotonic())

    assert not case.success
    assert [check.name for check in case.checks] == [
        "review/ephemeral-thread",
        "review/catalog-over-limit/startup-failed",
        "review/catalog-over-limit/exact-limit-error",
        "review/catalog-over-limit/configured-server",
        "review/catalog-over-limit/server-info",
        "review/catalog-over-limit/tool-count",
    ]
    assert [check.name for check in case.checks if not check.success] == [
        "review/catalog-over-limit/startup-failed",
        "review/catalog-over-limit/exact-limit-error",
        "review/catalog-over-limit/server-info",
        "review/catalog-over-limit/tool-count",
    ]


def test_catalog_over_limit_does_not_treat_a_missing_notification_as_rejection(
    tmp_path: Path,
) -> None:
    case = CaseResult(transport="review-stdio:catalog-over-limit", mode=LEGACY_VERSION)
    client = _CatalogBoundaryClient(
        status=None,
        tool_count=0,
        server_info=None,
    )

    _run_catalog_boundary_checks(
        case,
        client,  # type: ignore[arg-type] - exercise the real app-server client contract.
        profile=CATALOG_OVER_LIMIT_PROFILE,
        workspace=tmp_path,
    )
    case.finish(time.monotonic())

    assert not case.success
    assert [check.name for check in case.checks if not check.success] == [
        "review/catalog-over-limit/startup-failed"
    ]


@pytest.mark.parametrize(
    ("profile", "status", "failure_name"),
    [
        (
            CATALOG_MAX_PROFILE,
            "ready",
            "review/catalog-at-limit/startup-ready",
        ),
        (
            CATALOG_OVER_LIMIT_PROFILE,
            "failed",
            "review/catalog-over-limit/startup-failed",
        ),
    ],
)
def test_catalog_boundary_does_not_accept_another_servers_startup_notification(
    profile: str, status: str, failure_name: str, tmp_path: Path
) -> None:
    at_limit = profile == CATALOG_MAX_PROFILE
    case = CaseResult(transport=f"review-stdio:{profile}", mode=MODERN_VERSION)
    client = _CatalogBoundaryClient(
        status=status,
        tool_count=MAX_CATALOG_ITEMS if at_limit else 0,
        server_info={"name": "catalog-boundary-server"} if at_limit else None,
        error=CATALOG_LIMIT_ERROR if not at_limit else None,
        notification_name="another-mcp-server",
    )

    _run_catalog_boundary_checks(
        case,
        client,  # type: ignore[arg-type] - exercise the real app-server client contract.
        profile=profile,
        workspace=tmp_path,
    )
    case.finish(time.monotonic())

    assert not case.success
    assert [check.name for check in case.checks if not check.success] == [failure_name]


@pytest.mark.parametrize("tool_count", [MAX_CATALOG_ITEMS - 1, MAX_CATALOG_ITEMS + 1])
def test_catalog_at_limit_rejects_incomplete_or_excessive_discovery(
    tool_count: int, tmp_path: Path
) -> None:
    case = CaseResult(transport="review-stdio:catalog-max", mode=MODERN_VERSION)
    client = _CatalogBoundaryClient(
        status="ready",
        tool_count=tool_count,
        server_info={"name": "catalog-boundary-server"},
    )

    _run_catalog_boundary_checks(
        case,
        client,  # type: ignore[arg-type] - exercise the real app-server client contract.
        profile=CATALOG_MAX_PROFILE,
        workspace=tmp_path,
    )
    case.finish(time.monotonic())

    assert not case.success
    assert [check.name for check in case.checks if not check.success] == [
        "review/catalog-at-limit/tool-count"
    ]


@pytest.mark.parametrize("mode", REVIEW_MODES)
@pytest.mark.parametrize("profile", CATALOG_BOUNDARY_PROFILES)
def test_catalog_boundary_stdio_registration_uses_the_exact_fixture_profile(
    mode: str, profile: str
) -> None:
    command = _review_registration_command(
        Path("/opt/codex"),
        Path("/src/server.py"),
        transport="stdio",
        mode=mode,
        profile=profile,
        http_url=None,
    )

    assert command[-6:] == [
        "--mode",
        mode,
        "--transport",
        "stdio",
        "--profile",
        profile,
    ]


def test_review_matrix_appends_all_transport_catalog_boundaries(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    observed: list[tuple[str, str, str]] = []

    def fake_run_review_case(
        codex_binary: Path,
        server_script: Path,
        *,
        mode: str,
        profile: str,
        transport: str,
        case_home: Path,
        timeout_seconds: float,
    ) -> CaseResult:
        observed.append((mode, profile, transport))
        case = CaseResult(transport=f"review-{transport}:{profile}", mode=mode)
        case.check("review/fake-preserved-case", True, "ok")
        case.finish(time.monotonic())
        return case

    monkeypatch.setattr("review_regressions._run_review_case", fake_run_review_case)
    report = run_review_regressions(
        Path("/opt/codex"),
        modes=REVIEW_MODES,
        server_script=Path("/src/server.py"),
        artifact_parent=tmp_path,
    )

    assert observed[:9] == [
        (SHIPPING_LEGACY_VERSION, REVIEW_PROFILE, "stdio"),
        (LEGACY_VERSION, REVIEW_PROFILE, "stdio"),
        (MODERN_VERSION, REVIEW_PROFILE, "stdio"),
        (MODERN_VERSION, REVIEW_PROFILE, "http"),
        (MODERN_VERSION, REPEATED_CURSOR_PROFILE, "http"),
        (MODERN_VERSION, MISMATCHED_DISCOVERY_ID_PROFILE, "http"),
        (MODERN_VERSION, NULL_DISCOVERY_ID_PROFILE, "http"),
        (MODERN_VERSION, SSE_CR_COMMENTS_PROFILE, "http"),
        (MODERN_VERSION, SSE_COMMENT_FLOOD_PROFILE, "http"),
    ]
    assert observed[9:] == [
        (mode, profile, transport)
        for mode in REVIEW_MODES
        for transport in CATALOG_BOUNDARY_TRANSPORTS
        for profile in CATALOG_BOUNDARY_PROFILES
    ]
    assert report["success"] is True
    assert report["summary"] == {
        "passed": 21,
        "failed": 0,
        "total": 21,
        "casesPassed": 21,
        "casesTotal": 21,
    }


_KNOWN_REVIEW_FAILURES = {
    (MODERN_VERSION, f"review-{transport}:{REVIEW_PROFILE}", check_id)
    for transport in ("stdio", "http")
    for check_id in (
        "review/exact-large-integer-elicitation-schema",
        "review/exact-large-integer-elicitation-round-trip",
        "review/mrtr-input-requests-bounded-to-64",
    )
}

_KNOWN_MAIN_CATALOG_FAILURES = {
    (mode, f"review-{transport}:{CATALOG_OVER_LIMIT_PROFILE}", check_id)
    for mode in REVIEW_MODES
    for transport in CATALOG_BOUNDARY_TRANSPORTS
    for check_id in (
        "review/catalog-over-limit/startup-failed",
        "review/catalog-over-limit/exact-limit-error",
        "review/catalog-over-limit/server-info",
        "review/catalog-over-limit/tool-count",
    )
}


def _refresh_review_gate_report(report: dict[str, object]) -> None:
    cases = report["cases"]
    assert isinstance(cases, list)
    passed = 0
    failed = 0
    cases_passed = 0
    for case in cases:
        assert isinstance(case, dict)
        checks = case["checks"]
        assert isinstance(checks, list)
        successes = []
        for check in checks:
            assert isinstance(check, dict)
            success = check["success"]
            assert isinstance(success, bool)
            successes.append(success)
        case_success = bool(checks) and all(successes)
        case["success"] = case_success
        passed += sum(successes)
        failed += len(successes) - sum(successes)
        cases_passed += case_success
    report["success"] = bool(cases) and failed == 0
    report["summary"] = {
        "passed": passed,
        "failed": failed,
        "total": passed + failed,
        "casesPassed": cases_passed,
        "casesTotal": len(cases),
    }


def _review_gate_report(
    *,
    failures: set[tuple[str, str, str]] | None = None,
) -> dict[str, object]:
    failures = _KNOWN_REVIEW_FAILURES if failures is None else failures
    cases: list[dict[str, object]] = []
    for mode, transport in sorted(_required_review_cases()):
        check_ids = ["review/mcp-registration"]
        if transport.endswith(f":{CATALOG_OVER_LIMIT_PROFILE}"):
            check_ids.extend(
                (
                    "review/catalog-over-limit/startup-failed",
                    "review/catalog-over-limit/exact-limit-error",
                    "review/catalog-over-limit/configured-server",
                    "review/catalog-over-limit/server-info",
                    "review/catalog-over-limit/tool-count",
                )
            )
        if mode == MODERN_VERSION and transport in {
            f"review-stdio:{REVIEW_PROFILE}",
            f"review-http:{REVIEW_PROFILE}",
        }:
            check_ids.extend(
                (
                    "review/exact-large-integer-elicitation-schema",
                    "review/exact-large-integer-elicitation-round-trip",
                    "review/mrtr-input-requests-bounded-to-64",
                )
            )
        cases.append(
            {
                "mode": mode,
                "transport": transport,
                "checks": [
                    {
                        "name": check_id,
                        "success": (mode, transport, check_id) not in failures,
                        "detail": "synthetic real-probe identity",
                    }
                    for check_id in check_ids
                ],
            }
        )
    report: dict[str, object] = {
        "schemaVersion": REVIEW_REPORT_SCHEMA_VERSION,
        "reviewer": REVIEWER,
        "modes": list(REVIEW_MODES),
        "cases": cases,
    }
    _refresh_review_gate_report(report)
    return report


def test_reviewer_baseline_preserves_all_required_cases_and_known_failures() -> None:
    baseline = _compact_review_regression_baseline(_review_gate_report())

    assert baseline["baselineKind"] == REVIEW_BASELINE_KIND
    assert baseline["requiredModes"] == list(REVIEW_MODES)
    assert len(baseline["requiredCases"]) == 21  # type: ignore[arg-type]
    checks = baseline["checks"]
    assert isinstance(checks, dict)
    failures = checks["failing"]
    assert isinstance(failures, list)
    assert {
        (failure["mode"], failure["transport"], failure["check_id"])
        for failure in failures
    } == _KNOWN_REVIEW_FAILURES


def test_committed_reviewer_baseline_records_all_real_production_checks() -> None:
    path = Path(__file__).with_name("review-regression-baseline-v1.json")
    baseline = json.loads(path.read_text(encoding="utf-8"))
    errors: list[str] = []
    checks = _required_review_checks(baseline, label="baseline", errors=errors)

    assert errors == []
    assert len(checks) == 186
    assert sum(checks.values()) == 156
    assert {
        (identity.mode, identity.transport, identity.check_id)
        for identity, success in checks.items()
        if not success
    } == _KNOWN_REVIEW_FAILURES | _KNOWN_MAIN_CATALOG_FAILURES
    assert baseline["summary"] == {
        "passed": 156,
        "failed": 30,
        "total": 186,
        "casesPassed": 13,
        "casesTotal": 21,
    }


def test_reviewer_gate_reports_known_failures_without_calling_them_passes() -> None:
    report = _review_gate_report()
    baseline = _compact_review_regression_baseline(report)

    gate = _evaluate_review_regression_gate(report, baseline)

    assert report["success"] is False
    assert gate["success"] is True
    assert gate["configurationErrors"] == []
    assert gate["newFailures"] == []
    assert gate["missingChecks"] == []
    assert gate["fixedChecks"] == []
    known = gate["knownFailures"]
    assert isinstance(known, list)
    assert {
        (failure["mode"], failure["transport"], failure["check_id"])
        for failure in known
    } == _KNOWN_REVIEW_FAILURES


def test_reviewer_gate_preserves_catalog_failures_observed_on_current_main() -> None:
    main_failures = _KNOWN_REVIEW_FAILURES | _KNOWN_MAIN_CATALOG_FAILURES
    report = _review_gate_report(failures=main_failures)
    baseline = _compact_review_regression_baseline(report)

    gate = _evaluate_review_regression_gate(report, baseline)

    assert report["success"] is False
    assert gate["success"] is True
    assert gate["configurationErrors"] == []
    assert gate["newFailures"] == []
    assert gate["missingChecks"] == []
    known = gate["knownFailures"]
    assert isinstance(known, list)
    assert {
        (failure["mode"], failure["transport"], failure["check_id"])
        for failure in known
    } == main_failures


def test_reviewer_gate_reports_all_future_catalog_boundary_fixes() -> None:
    main_failures = _KNOWN_REVIEW_FAILURES | _KNOWN_MAIN_CATALOG_FAILURES
    baseline = _compact_review_regression_baseline(
        _review_gate_report(failures=main_failures)
    )
    candidate = _review_gate_report()

    gate = _evaluate_review_regression_gate(candidate, baseline)

    assert gate["success"] is True
    assert gate["configurationErrors"] == []
    assert gate["newFailures"] == []
    assert gate["missingChecks"] == []
    fixed = gate["fixedChecks"]
    assert isinstance(fixed, list)
    assert {
        (identity["mode"], identity["transport"], identity["check_id"])
        for identity in fixed
    } == _KNOWN_MAIN_CATALOG_FAILURES
    known = gate["knownFailures"]
    assert isinstance(known, list)
    assert {
        (identity["mode"], identity["transport"], identity["check_id"])
        for identity in known
    } == _KNOWN_REVIEW_FAILURES


def test_reviewer_gate_rejects_a_newly_failing_previously_passing_check() -> None:
    baseline = _compact_review_regression_baseline(_review_gate_report())
    new_failure = (
        SHIPPING_LEGACY_VERSION,
        f"review-stdio:{REVIEW_PROFILE}",
        "review/mcp-registration",
    )
    candidate = _review_gate_report(failures=_KNOWN_REVIEW_FAILURES | {new_failure})

    gate = _evaluate_review_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert gate["newFailures"] == [
        {
            "mode": new_failure[0],
            "transport": new_failure[1],
            "check_id": new_failure[2],
        }
    ]


def test_reviewer_gate_reports_when_a_known_failure_is_fixed() -> None:
    baseline = _compact_review_regression_baseline(_review_gate_report())
    fixed = sorted(_KNOWN_REVIEW_FAILURES)[0]
    candidate = _review_gate_report(failures=_KNOWN_REVIEW_FAILURES - {fixed})

    gate = _evaluate_review_regression_gate(candidate, baseline)

    assert gate["success"] is True
    assert gate["fixedChecks"] == [
        {"mode": fixed[0], "transport": fixed[1], "check_id": fixed[2]}
    ]
    assert len(gate["knownFailures"]) == 5  # type: ignore[arg-type]


def test_reviewer_gate_rejects_an_omitted_required_case() -> None:
    baseline = _compact_review_regression_baseline(_review_gate_report())
    candidate = _review_gate_report()
    cases = candidate["cases"]
    assert isinstance(cases, list)
    omitted = cases.pop(0)
    assert isinstance(omitted, dict)
    _refresh_review_gate_report(candidate)

    gate = _evaluate_review_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any(
        "missing required reviewer case" in error
        for error in gate["configurationErrors"]  # type: ignore[union-attr]
    )
    assert any(
        missing["mode"] == omitted["mode"]
        and missing["transport"] == omitted["transport"]
        for missing in gate["missingChecks"]  # type: ignore[union-attr]
    )


def test_reviewer_gate_rejects_an_omitted_required_check() -> None:
    baseline = _compact_review_regression_baseline(_review_gate_report())
    candidate = _review_gate_report()
    cases = candidate["cases"]
    assert isinstance(cases, list)
    case = next(
        case
        for case in cases
        if isinstance(case, dict)
        and case["mode"] == MODERN_VERSION
        and case["transport"] == f"review-stdio:{REVIEW_PROFILE}"
    )
    checks = case["checks"]
    assert isinstance(checks, list)
    omitted = checks.pop(0)
    assert isinstance(omitted, dict)
    _refresh_review_gate_report(candidate)

    gate = _evaluate_review_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert gate["missingChecks"] == [
        {
            "mode": MODERN_VERSION,
            "transport": f"review-stdio:{REVIEW_PROFILE}",
            "check_id": omitted["name"],
        }
    ]


@pytest.mark.parametrize("field", ["schemaVersion", "reviewer", "requiredModes"])
def test_reviewer_gate_rejects_an_incompatible_baseline(field: str) -> None:
    baseline = _compact_review_regression_baseline(_review_gate_report())
    baseline[field] = [] if field == "requiredModes" else "wrong"

    gate = _evaluate_review_regression_gate(_review_gate_report(), baseline)

    assert gate["success"] is False
    assert gate["configurationErrors"]


def test_reviewer_gate_rejects_duplicate_baseline_check_identities() -> None:
    baseline = _compact_review_regression_baseline(_review_gate_report())
    buckets = baseline["checks"]
    assert isinstance(buckets, dict)
    passing = buckets["passing"]
    assert isinstance(passing, list)
    passing.append(deepcopy(passing[0]))

    gate = _evaluate_review_regression_gate(_review_gate_report(), baseline)

    assert gate["success"] is False
    assert any(
        "duplicate reviewer check" in error
        for error in gate["configurationErrors"]  # type: ignore[union-attr]
    )


def test_reviewer_gate_rejects_inconsistent_candidate_totals() -> None:
    baseline = _compact_review_regression_baseline(_review_gate_report())
    candidate = _review_gate_report()
    candidate["summary"] = {"passed": 999, "failed": 0, "total": 999}

    gate = _evaluate_review_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any(
        "inconsistent reviewer check totals" in error
        for error in gate["configurationErrors"]  # type: ignore[union-attr]
    )


def test_review_json_is_pretty_sorted_and_newline_terminated(tmp_path: Path) -> None:
    report_path = tmp_path / "nested" / "review.json"

    _write_review_json(report_path, {"z": 1, "a": {"second": 2, "first": 1}})

    assert report_path.read_text(encoding="utf-8") == (
        '{\n  "a": {\n    "first": 1,\n    "second": 2\n  },\n  "z": 1\n}\n'
    )


def test_review_cli_supports_a_reviewed_production_baseline() -> None:
    args = _parse_args(
        [
            "/opt/codex",
            "--baseline-report",
            "/tmp/review-regression-baseline-v1.json",
            "--report",
            "/tmp/mcp-review-regressions.json",
        ]
    )

    assert args.baseline_report == Path("/tmp/review-regression-baseline-v1.json")


def test_review_cli_requires_a_source_report_when_extracting_a_baseline(
    capsys: pytest.CaptureFixture[str],
) -> None:
    result = main(
        [sys.executable, "--extract-baseline", "/tmp/review-baseline-test.json"]
    )

    assert result == 2
    assert "--extract-baseline requires --baseline-report" in capsys.readouterr().err


def test_review_cli_exits_successfully_only_for_a_passing_regression_gate(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    candidate = _review_gate_report()
    baseline_path = tmp_path / "review-baseline.json"
    report_path = tmp_path / "review-report.json"
    _write_review_json(
        baseline_path, _compact_review_regression_baseline(deepcopy(candidate))
    )
    monkeypatch.setattr(
        "review_regressions.run_review_regressions",
        lambda *args, **kwargs: deepcopy(candidate),
    )

    result = main(
        [
            sys.executable,
            "--baseline-report",
            str(baseline_path),
            "--report",
            str(report_path),
        ]
    )

    report = json.loads(report_path.read_text(encoding="utf-8"))
    assert result == 0
    assert report["success"] is False
    assert report["regressionGate"]["success"] is True
    assert len(report["regressionGate"]["knownFailures"]) == 6


def test_review_cli_does_not_accept_new_failures_as_known_regressions(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    baseline_path = tmp_path / "review-baseline.json"
    report_path = tmp_path / "review-report.json"
    _write_review_json(
        baseline_path, _compact_review_regression_baseline(_review_gate_report())
    )
    new_failure = (
        SHIPPING_LEGACY_VERSION,
        f"review-stdio:{REVIEW_PROFILE}",
        "review/mcp-registration",
    )
    monkeypatch.setattr(
        "review_regressions.run_review_regressions",
        lambda *args, **kwargs: _review_gate_report(
            failures=_KNOWN_REVIEW_FAILURES | {new_failure}
        ),
    )

    result = main(
        [
            sys.executable,
            "--baseline-report",
            str(baseline_path),
            "--report",
            str(report_path),
        ]
    )

    report = json.loads(report_path.read_text(encoding="utf-8"))
    assert result == 1
    assert report["success"] is False
    assert report["regressionGate"]["success"] is False
    assert len(report["regressionGate"]["newFailures"]) == 1


def test_review_cli_extracts_a_deterministic_complete_baseline(
    tmp_path: Path,
) -> None:
    source_path = tmp_path / "full-review-report.json"
    baseline_path = tmp_path / "review-regression-baseline-v1.json"
    report = _review_gate_report()
    _write_review_json(source_path, report)

    result = main(
        [
            sys.executable,
            "--baseline-report",
            str(source_path),
            "--extract-baseline",
            str(baseline_path),
        ]
    )

    expected = _compact_review_regression_baseline(report)
    assert result == 0
    assert baseline_path.read_text(encoding="utf-8") == (
        json.dumps(expected, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    )
