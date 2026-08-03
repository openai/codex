import os
import sys
import json
from copy import deepcopy
from pathlib import Path

import pytest
from run_codex_compliance import (
    COMPACT_REGRESSION_BASELINE_KIND,
    LEGACY_VERSION,
    MODERN_VERSION,
    OFFICIAL_CONFORMANCE_GIT_REF,
    OFFICIAL_CONFORMANCE_REPOSITORY,
    REPORT_SCHEMA_VERSION,
    REQUIRED_REGRESSION_MODES,
    RESOURCE_URIS,
    SERVER_NAME,
    SERVER_VERSION,
    SHIPPING_LEGACY_VERSION,
    TEST_SERVER_NAME,
    CaseResult,
    CheckResult,
    OfficialScenarioResult,
    _build_test_matrix,
    _compact_regression_baseline,
    _evaluate_regression_gate,
    _isolated_environment,
    _parse_args,
    _registration_command,
    _run_official_case,
    _scenario_ratio,
    _summarize_modes,
    _validate_inventory,
    _validate_registration,
    _write_compact_regression_baseline,
    _write_json_file,
    main,
    scenarios_for_mode,
)


def test_registration_commands_select_stdio_then_http_shapes() -> None:
    codex = Path("/opt/codex")
    server = Path("/src/server.py")

    stdio = _registration_command(
        codex,
        server,
        transport="stdio",
        mode=MODERN_VERSION,
        http_url=None,
    )
    http = _registration_command(
        codex,
        server,
        transport="http",
        mode=MODERN_VERSION,
        http_url="http://127.0.0.1:8765/mcp",
    )

    assert stdio == [
        "/opt/codex",
        "mcp",
        "add",
        TEST_SERVER_NAME,
        "--env",
        f"CODEX_MCP_PROTOCOL_VERSION={MODERN_VERSION}",
        "--",
        sys.executable,
        "/src/server.py",
        "--mode",
        MODERN_VERSION,
        "--transport",
        "stdio",
    ]
    assert http == [
        "/opt/codex",
        "mcp",
        "add",
        TEST_SERVER_NAME,
        "--url",
        "http://127.0.0.1:8765/mcp",
    ]


def test_registration_validation_checks_transport_and_mode() -> None:
    valid, _ = _validate_registration(
        {
            "name": TEST_SERVER_NAME,
            "enabled": True,
            "transport": {
                "type": "stdio",
                "command": sys.executable,
                "args": [
                    "/src/server.py",
                    "--mode",
                    LEGACY_VERSION,
                    "--transport",
                    "stdio",
                ],
            },
        },
        transport="stdio",
        mode=LEGACY_VERSION,
        http_url=None,
    )
    assert valid

    valid, _ = _validate_registration(
        {
            "name": TEST_SERVER_NAME,
            "enabled": True,
            "transport": {
                "type": "stdio",
                "command": sys.executable,
                "args": [
                    "/src/server.py",
                    "--mode",
                    MODERN_VERSION,
                    "--transport",
                    "stdio",
                ],
                "env": {"CODEX_MCP_PROTOCOL_VERSION": MODERN_VERSION},
            },
        },
        transport="stdio",
        mode=MODERN_VERSION,
        http_url=None,
    )
    assert valid

    valid, detail = _validate_registration(
        {
            "name": TEST_SERVER_NAME,
            "enabled": True,
            "transport": {
                "type": "stdio",
                "command": sys.executable,
                "args": [
                    "/src/server.py",
                    "--mode",
                    MODERN_VERSION,
                    "--transport",
                    "stdio",
                ],
            },
        },
        transport="stdio",
        mode=MODERN_VERSION,
        http_url=None,
    )
    assert not valid
    assert "protocol opt-in" in detail

    valid, detail = _validate_registration(
        {
            "name": TEST_SERVER_NAME,
            "enabled": True,
            "transport": {
                "type": "streamable_http",
                "url": "http://127.0.0.1:2/mcp",
            },
        },
        transport="http",
        mode=MODERN_VERSION,
        http_url="http://127.0.0.1:1/mcp",
    )
    assert not valid
    assert "unexpected HTTP transport" in detail


def test_inventory_validation_requires_modern_tool_and_all_pages() -> None:
    inventory = {
        "data": [
            {
                "name": TEST_SERVER_NAME,
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": SERVER_VERSION,
                },
                "tools": {
                    name: {"name": name}
                    for name in (
                        "echo",
                        "client_metadata",
                        "progress",
                        "request_input",
                    )
                },
                "resources": [{"uri": uri} for uri in RESOURCE_URIS],
            }
        ]
    }

    valid, _ = _validate_inventory(inventory, mode=MODERN_VERSION)
    assert valid

    inventory["data"][0]["tools"].pop("request_input")
    inventory["data"][0]["resources"].pop()
    valid, detail = _validate_inventory(inventory, mode=MODERN_VERSION)
    assert not valid
    assert "request_input" in detail
    assert RESOURCE_URIS[1] in detail


def test_isolated_environment_drops_model_credentials(
    tmp_path: Path,
    monkeypatch,
) -> None:
    monkeypatch.setenv("CODEX_API_KEY", "secret")
    monkeypatch.setenv("CODEX_ACCESS_TOKEN", "secret")
    monkeypatch.setenv("OPENAI_API_KEY", "secret")

    env = _isolated_environment(tmp_path)

    assert env["CODEX_HOME"] == str(tmp_path)
    assert "CODEX_API_KEY" not in env
    assert "CODEX_ACCESS_TOKEN" not in env
    assert "OPENAI_API_KEY" not in env
    assert env.get("PATH") == os.environ.get("PATH")


def test_mode_summaries_separate_official_and_supplemental_percentages() -> None:
    legacy = CaseResult(
        transport="stdio",
        mode=LEGACY_VERSION,
        success=True,
        checks=[
            CheckResult("mcp_add", True, "ok"),
            CheckResult("inventory", True, "ok"),
            CheckResult("echo_tool", True, "ok"),
        ],
    )
    modern = CaseResult(
        transport="http",
        mode=MODERN_VERSION,
        success=False,
        checks=[
            CheckResult("mcp_add", True, "ok"),
            CheckResult("modern_feature_enablement", True, "ok"),
            CheckResult("inventory", False, "failed"),
            CheckResult("http_header_mirroring", False, "failed"),
            CheckResult(
                "official/example/pass",
                True,
                "ok",
                source="official",
                category="non-auth",
                scenario="example-pass",
            ),
            CheckResult(
                "official/example/skipped",
                True,
                "not exercised",
                status="SKIP",
                source="official",
                category="auth",
                scenario="example-skipped",
            ),
        ],
    )

    summaries = _summarize_modes([legacy, modern])

    assert summaries == [
        {
            "mode": LEGACY_VERSION,
            "checks": {"passed": 3, "total": 3, "percentage": 100.0},
            "officialChecks": {"passed": 0, "total": 0, "percentage": 0.0},
            "officialNonAuthChecks": {
                "passed": 0,
                "total": 0,
                "percentage": 0.0,
            },
            "officialAuthChecks": {
                "passed": 0,
                "total": 0,
                "percentage": 0.0,
            },
            "officialNonAuthScenarios": {
                "passed": 0,
                "total": 0,
                "percentage": 0.0,
            },
            "officialAuthScenarios": {
                "passed": 0,
                "total": 0,
                "percentage": 0.0,
            },
            "harnessChecks": {"passed": 0, "total": 0, "percentage": 0.0},
            "supplementalChecks": {
                "passed": 3,
                "total": 3,
                "percentage": 100.0,
            },
            "transportCases": {"passed": 1, "total": 1, "percentage": 100.0},
        },
        {
            "mode": MODERN_VERSION,
            "checks": {"passed": 3, "total": 5, "percentage": 60.0},
            "officialChecks": {
                "passed": 1,
                "total": 1,
                "percentage": 100.0,
            },
            "officialNonAuthChecks": {
                "passed": 1,
                "total": 1,
                "percentage": 100.0,
            },
            "officialAuthChecks": {
                "passed": 0,
                "total": 0,
                "percentage": 0.0,
            },
            "officialNonAuthScenarios": {
                "passed": 1,
                "total": 1,
                "percentage": 100.0,
            },
            "officialAuthScenarios": {
                "passed": 1,
                "total": 1,
                "percentage": 100.0,
            },
            "harnessChecks": {"passed": 0, "total": 0, "percentage": 0.0},
            "supplementalChecks": {
                "passed": 2,
                "total": 4,
                "percentage": 50.0,
            },
            "transportCases": {"passed": 0, "total": 1, "percentage": 0.0},
        },
    ]


def test_matrix_reports_pass_fail_and_not_applicable() -> None:
    legacy = CaseResult(
        transport="stdio",
        mode=LEGACY_VERSION,
        checks=[
            CheckResult("inventory", True, "ok"),
            CheckResult("echo_tool", True, "ok"),
        ],
    )
    modern = CaseResult(
        transport="http",
        mode=MODERN_VERSION,
        checks=[
            CheckResult("inventory", False, "failed"),
            CheckResult("http_header_mirroring", False, "failed"),
        ],
    )

    matrix = _build_test_matrix([legacy, modern])

    assert matrix["columns"] == [
        {
            "key": f"stdio:{LEGACY_VERSION}",
            "transport": "stdio",
            "mode": LEGACY_VERSION,
        },
        {
            "key": f"http:{MODERN_VERSION}",
            "transport": "http",
            "mode": MODERN_VERSION,
        },
    ]
    assert matrix["rows"] == [
        {
            "test": "inventory",
            "results": {
                f"stdio:{LEGACY_VERSION}": "PASS",
                f"http:{MODERN_VERSION}": "FAIL",
            },
        },
        {
            "test": "echo_tool",
            "results": {
                f"stdio:{LEGACY_VERSION}": "PASS",
                f"http:{MODERN_VERSION}": "N/A",
            },
        },
        {
            "test": "http_header_mirroring",
            "results": {
                f"stdio:{LEGACY_VERSION}": "N/A",
                f"http:{MODERN_VERSION}": "FAIL",
            },
        },
    ]


def test_scenario_ratio_includes_adapter_failures() -> None:
    checks = [
        CheckResult(
            "official/auth/example/assertion",
            True,
            "ok",
            source="official",
            scenario="auth/example",
            category="auth",
        ),
        CheckResult(
            "harness/auth/example/codex-adapter",
            False,
            "adapter failed",
            source="harness",
            scenario="auth/example",
            category="auth",
        ),
        CheckResult(
            "official/auth/other/assertion",
            True,
            "ok",
            source="official",
            scenario="auth/other",
            category="auth",
        ),
    ]

    assert _scenario_ratio(checks, category="auth") == {
        "passed": 1,
        "total": 2,
        "percentage": 50.0,
    }


@pytest.mark.parametrize("mode", [LEGACY_VERSION, MODERN_VERSION])
@pytest.mark.parametrize(
    ("adapter_success", "expected_status"),
    [(False, "FAIL"), (True, "PASS")],
)
def test_official_case_preserves_adapter_check_identity_on_failure_and_success(
    mode: str,
    adapter_success: bool,
    expected_status: str,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    scenario = "auth/pre-registration"
    result = OfficialScenarioResult(
        scenario=scenario,
        success=adapter_success,
        adapter_success=adapter_success,
        adapter_detail="adapter succeeded" if adapter_success else "adapter failed",
    )
    monkeypatch.setattr(
        "run_codex_compliance.run_official_mode",
        lambda **_kwargs: [result],
    )

    case = _run_official_case(
        Path("/opt/codex"),
        Path("/src/codex_conformance_adapter.py"),
        conformance_command=["conformance"],
        mode=mode,
        scenarios=[scenario],
        case_home=tmp_path / mode,
        timeout_seconds=1.0,
        enable_modern_feature=True,
    )

    assert case.checks == [
        CheckResult(
            name=f"harness/{scenario}/codex-adapter",
            success=adapter_success,
            detail=result.adapter_detail,
            status=expected_status,
            source="harness",
            scenario=scenario,
            category="auth",
        )
    ]
    assert case.success is adapter_success


@pytest.mark.parametrize("mode", [LEGACY_VERSION, MODERN_VERSION])
def test_official_case_preserves_independent_runner_failure_with_successful_adapter(
    mode: str,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    scenario = "auth/pre-registration"
    result = OfficialScenarioResult(
        scenario=scenario,
        success=False,
        adapter_success=True,
        adapter_detail="adapter succeeded",
        runner_detail="official runner failed",
    )
    monkeypatch.setattr(
        "run_codex_compliance.run_official_mode",
        lambda **_kwargs: [result],
    )

    case = _run_official_case(
        Path("/opt/codex"),
        Path("/src/codex_conformance_adapter.py"),
        conformance_command=["conformance"],
        mode=mode,
        scenarios=[scenario],
        case_home=tmp_path / mode,
        timeout_seconds=1.0,
        enable_modern_feature=True,
    )

    assert case.checks == [
        CheckResult(
            name=f"harness/{scenario}/codex-adapter",
            success=True,
            detail="adapter succeeded",
            status="PASS",
            source="harness",
            scenario=scenario,
            category="auth",
        ),
        CheckResult(
            name=f"harness/{scenario}/official-runner",
            success=False,
            detail="official runner failed",
            status="FAIL",
            source="harness",
            scenario=scenario,
            category="auth",
        ),
    ]
    assert case.success is False
    assert case.diagnostics == f"{scenario}:\nofficial runner failed"


def _regression_report() -> dict[str, object]:
    cases: list[dict[str, object]] = []
    known_modern = {
        "auth/metadata-var3": "authorization-server-metadata",
        "auth/scope-from-www-authenticate": "scope-from-www-authenticate",
        "auth/scope-step-up": "scope-step-up-initial",
        "auth/pre-registration": "pre-registration-auth",
    }
    scenarios = {
        mode: list(scenarios_for_mode(mode, include_auth=True))
        for mode in REQUIRED_REGRESSION_MODES
    }

    for mode in REQUIRED_REGRESSION_MODES:
        cases.append(
            {
                "mode": mode,
                "transport": "stdio",
                "success": True,
                "checks": [
                    {
                        "name": "mcp_add",
                        "success": True,
                        "status": "PASS",
                        "source": "supplemental",
                        "scenario": None,
                        "check_id": None,
                    }
                ],
            }
        )
        official_checks: list[dict[str, object]] = []
        for scenario in scenarios[mode]:
            if mode == MODERN_VERSION and scenario == "request-metadata":
                official_checks.append(
                    {
                        "name": "harness/request-metadata/official-runner",
                        "success": False,
                        "status": "FAIL",
                        "source": "harness",
                        "scenario": scenario,
                        "check_id": None,
                    }
                )
                continue

            known = mode == MODERN_VERSION and scenario in known_modern
            official_checks.append(
                {
                    "name": f"official/{scenario}/assertion",
                    "success": not known,
                    "status": "FAIL" if known else "PASS",
                    "source": "official",
                    "scenario": scenario,
                    "check_id": (
                        known_modern[scenario] if known else f"assertion-{scenario}"
                    ),
                }
            )
            if known:
                official_checks.append(
                    {
                        "name": f"harness/{scenario}/codex-adapter",
                        "success": False,
                        "status": "FAIL",
                        "source": "harness",
                        "scenario": scenario,
                        "check_id": None,
                    }
                )
        cases.append(
            {
                "mode": mode,
                "transport": "official-http",
                "success": mode == SHIPPING_LEGACY_VERSION,
                "checks": official_checks,
            }
        )

    return {
        "schemaVersion": REPORT_SCHEMA_VERSION,
        "success": False,
        "versionCheck": {"success": True},
        "modernFeatureEnablement": True,
        "automaticAuthRequired": False,
        "officialConformance": {
            "repository": OFFICIAL_CONFORMANCE_REPOSITORY,
            "gitRef": OFFICIAL_CONFORMANCE_GIT_REF,
            "authenticationIncluded": True,
            "scenarios": scenarios,
        },
        "cases": cases,
    }


def _regression_case(
    report: dict[str, object],
    mode: str,
    transport: str,
) -> dict[str, object]:
    cases = report["cases"]
    assert isinstance(cases, list)
    return next(
        case
        for case in cases
        if isinstance(case, dict)
        and case["mode"] == mode
        and case["transport"] == transport
    )


def _regression_scenario_check(
    report: dict[str, object],
    *,
    mode: str = MODERN_VERSION,
    scenario: str,
    source: str = "official",
) -> dict[str, object]:
    checks = _regression_case(report, mode, "official-http")["checks"]
    assert isinstance(checks, list)
    return next(
        check
        for check in checks
        if isinstance(check, dict)
        and check["scenario"] == scenario
        and check["source"] == source
    )


def test_regression_gate_preserves_truthful_known_modern_failures() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)

    gate = _evaluate_regression_gate(candidate, baseline)

    assert candidate["success"] is False
    assert gate["success"] is True
    assert gate["requiredModes"] == [
        SHIPPING_LEGACY_VERSION,
        LEGACY_VERSION,
        MODERN_VERSION,
    ]
    assert len(gate["knownFailures"]) == 9
    assert gate["newFailures"] == []
    assert gate["missingChecks"] == []
    assert gate["configurationErrors"] == []


def test_regression_gate_rejects_a_new_modern_failure() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    check = _regression_scenario_check(candidate, scenario="tools_call")
    check["success"] = False
    check["status"] = "FAIL"

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any(item["scenario"] == "tools_call" for item in gate["newFailures"])


def test_regression_gate_rejects_a_new_intermediate_oauth_failure() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    check = _regression_scenario_check(
        candidate,
        mode=LEGACY_VERSION,
        scenario="auth/metadata-default",
    )
    check["success"] = False
    check["status"] = "FAIL"

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any(
        item["mode"] == LEGACY_VERSION and item["scenario"] == "auth/metadata-default"
        for item in gate["newFailures"]
    )


def test_regression_gate_rejects_new_failures_inside_a_known_failing_scenario() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    checks = _regression_case(candidate, MODERN_VERSION, "official-http")["checks"]
    assert isinstance(checks, list)
    checks.append(
        {
            "name": "official/auth/metadata-var3/new assertion",
            "success": False,
            "status": "FAIL",
            "source": "official",
            "scenario": "auth/metadata-var3",
            "check_id": "previously-passing-metadata-assertion",
        }
    )

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any(
        item["check_id"] == "previously-passing-metadata-assertion"
        for item in gate["newFailures"]
    )


def test_regression_gate_accepts_fixed_known_failures() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    check = _regression_scenario_check(candidate, scenario="auth/metadata-var3")
    check["success"] = True
    check["status"] = "PASS"

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is True
    assert any(
        item["check_id"] == "authorization-server-metadata"
        for item in gate["fixedChecks"]
    )


def test_regression_gate_accepts_fixed_oauth_official_and_adapter_checks() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    scenario = "auth/pre-registration"
    for source in ("official", "harness"):
        check = _regression_scenario_check(
            candidate,
            scenario=scenario,
            source=source,
        )
        check["success"] = True
        check["status"] = "PASS"

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is True
    assert gate["configurationErrors"] == []
    assert gate["newFailures"] == []
    assert gate["missingChecks"] == []
    assert gate["fixedChecks"] == [
        {
            "mode": MODERN_VERSION,
            "transport": "official-http",
            "source": "harness",
            "scenario": scenario,
            "check_id": f"harness/{scenario}/codex-adapter",
        },
        {
            "mode": MODERN_VERSION,
            "transport": "official-http",
            "source": "official",
            "scenario": scenario,
            "check_id": "pre-registration-auth",
        },
    ]


def test_regression_gate_rejects_a_missing_fixed_oauth_adapter_check() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    scenario = "auth/pre-registration"
    check = _regression_scenario_check(candidate, scenario=scenario)
    check["success"] = True
    check["status"] = "PASS"
    checks = _regression_case(candidate, MODERN_VERSION, "official-http")["checks"]
    assert isinstance(checks, list)
    checks[:] = [
        item
        for item in checks
        if item.get("scenario") != scenario or item.get("source") != "harness"
    ]

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert gate["configurationErrors"] == []
    assert gate["newFailures"] == []
    assert gate["missingChecks"] == [
        {
            "mode": MODERN_VERSION,
            "transport": "official-http",
            "source": "harness",
            "scenario": scenario,
            "check_id": f"harness/{scenario}/codex-adapter",
        }
    ]
    assert gate["fixedChecks"] == [
        {
            "mode": MODERN_VERSION,
            "transport": "official-http",
            "source": "official",
            "scenario": scenario,
            "check_id": "pre-registration-auth",
        }
    ]


def test_regression_gate_rejects_a_skipped_previous_failure() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    check = _regression_scenario_check(candidate, scenario="auth/metadata-var3")
    check["success"] = True
    check["status"] = "SKIP"

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any(
        item["check_id"] == "authorization-server-metadata"
        for item in gate["missingChecks"]
    )


def test_regression_gate_rejects_a_missing_previously_passing_check() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    checks = _regression_case(candidate, MODERN_VERSION, "official-http")["checks"]
    assert isinstance(checks, list)
    checks[:] = [check for check in checks if check.get("scenario") != "tools_call"]

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any(item["scenario"] == "tools_call" for item in gate["missingChecks"])
    assert any("did not observe" in item for item in gate["configurationErrors"])


@pytest.mark.parametrize("mode", REQUIRED_REGRESSION_MODES)
@pytest.mark.parametrize("transport", ["stdio", "official-http"])
def test_regression_gate_requires_every_version_and_both_transports(
    mode: str,
    transport: str,
) -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    cases = candidate["cases"]
    assert isinstance(cases, list)
    cases[:] = [
        case
        for case in cases
        if case.get("mode") != mode or case.get("transport") != transport
    ]

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any(
        f"missing the required {transport} case for {mode}" in item
        for item in gate["configurationErrors"]
    )


def test_regression_gate_requires_the_complete_modern_authenticated_catalog() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    official = candidate["officialConformance"]
    assert isinstance(official, dict)
    scenarios = official["scenarios"]
    assert isinstance(scenarios, dict)
    scenarios[MODERN_VERSION] = [
        scenario
        for scenario in scenarios[MODERN_VERSION]
        if scenario != "auth/pre-registration"
    ]

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any("complete authenticated" in item for item in gate["configurationErrors"])


def test_regression_gate_requires_modern_feature_enablement() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    candidate["modernFeatureEnablement"] = False

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any("modern MCP feature" in item for item in gate["configurationErrors"])


def test_regression_gate_requires_the_same_pinned_upstream_suite() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    official = candidate["officialConformance"]
    assert isinstance(official, dict)
    official["gitRef"] = "0" * 40

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any("pinned upstream" in item for item in gate["configurationErrors"])


def test_regression_gate_requires_oauth_scenarios() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    official = candidate["officialConformance"]
    assert isinstance(official, dict)
    official["authenticationIncluded"] = False

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any("required OAuth" in item for item in gate["configurationErrors"])


def test_regression_gate_rejects_different_production_oauth_policies() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    candidate["automaticAuthRequired"] = True

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any(
        "different production OAuth" in item for item in gate["configurationErrors"]
    )


def test_regression_gate_rejects_unsupported_report_schema() -> None:
    baseline = _regression_report()
    baseline["schemaVersion"] = REPORT_SCHEMA_VERSION + 1

    gate = _evaluate_regression_gate(_regression_report(), baseline)

    assert gate["success"] is False
    assert any("report schema" in item for item in gate["configurationErrors"])


def test_regression_gate_rejects_any_shipping_failure() -> None:
    baseline = _regression_report()
    candidate = deepcopy(baseline)
    check = _regression_scenario_check(
        candidate,
        mode=SHIPPING_LEGACY_VERSION,
        scenario="initialize",
    )
    check["success"] = False
    check["status"] = "FAIL"

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any(
        "shipping MCP check failed" in item for item in gate["configurationErrors"]
    )


def test_regression_gate_ignores_retry_observation_renumbering() -> None:
    baseline = _regression_report()
    check = _regression_scenario_check(baseline, scenario="tools_call")
    check["name"] = "official/tools_call/authorization metadata#4"
    candidate = deepcopy(baseline)
    candidate_check = _regression_scenario_check(candidate, scenario="tools_call")
    candidate_check["name"] = "official/tools_call/authorization metadata#57"
    checks = _regression_case(candidate, MODERN_VERSION, "official-http")["checks"]
    assert isinstance(checks, list)
    repeated = deepcopy(candidate_check)
    repeated["name"] = "official/tools_call/authorization metadata#58"
    checks.append(repeated)

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is True
    assert gate["newFailures"] == []
    assert gate["missingChecks"] == []


def test_regression_cli_accepts_a_baseline_report() -> None:
    args = _parse_args(
        ["/opt/codex", "--baseline-report", "/tmp/codex-mcp-baseline.json"]
    )

    assert args.baseline_report == Path("/tmp/codex-mcp-baseline.json")
    assert args.mode == "all"
    assert args.auth is True
    assert args.enable_modern_feature is True


def test_regression_gate_accepts_a_compact_baseline() -> None:
    report = _regression_report()

    compact = _compact_regression_baseline(report)
    gate = _evaluate_regression_gate(deepcopy(report), compact)

    assert compact["baselineKind"] == COMPACT_REGRESSION_BASELINE_KIND
    assert compact["requiredModes"] == [
        SHIPPING_LEGACY_VERSION,
        LEGACY_VERSION,
        MODERN_VERSION,
    ]
    assert compact["transports"] == ["stdio", "official-http"]
    assert "cases" not in compact
    assert gate["success"] is True
    assert len(gate["knownFailures"]) == 9


def test_regression_gate_accepts_the_real_in_memory_tuple_scenario_catalog() -> None:
    baseline = _compact_regression_baseline(_regression_report())
    candidate = _regression_report()
    official = candidate["officialConformance"]
    assert isinstance(official, dict)
    scenarios = official["scenarios"]
    assert isinstance(scenarios, dict)
    official["scenarios"] = {
        mode: tuple(selected) for mode, selected in scenarios.items()
    }

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is True
    assert gate["configurationErrors"] == []


def test_regression_gate_rejects_a_new_failure_against_a_compact_baseline() -> None:
    baseline = _compact_regression_baseline(_regression_report())
    candidate = _regression_report()
    check = _regression_scenario_check(candidate, scenario="tools_call")
    check["success"] = False
    check["status"] = "FAIL"

    gate = _evaluate_regression_gate(candidate, baseline)

    assert gate["success"] is False
    assert any(item["scenario"] == "tools_call" for item in gate["newFailures"])


def test_regression_gate_rejects_malformed_compact_identities() -> None:
    compact = _compact_regression_baseline(_regression_report())
    checks = compact["checks"]
    assert isinstance(checks, dict)
    passing = checks["passing"]
    assert isinstance(passing, list)
    passing[0]["check_id"] = ""

    gate = _evaluate_regression_gate(_regression_report(), compact)

    assert gate["success"] is False
    assert any(
        "malformed passing baseline identity" in e for e in gate["configurationErrors"]
    )


def test_regression_gate_rejects_an_unknown_compact_baseline_format() -> None:
    compact = _compact_regression_baseline(_regression_report())
    compact["baselineKind"] = "unrecognized-baseline"

    gate = _evaluate_regression_gate(_regression_report(), compact)

    assert gate["success"] is False
    assert any("unsupported compact baseline" in e for e in gate["configurationErrors"])


def test_json_artifacts_are_sorted_human_readable_and_newline_terminated(
    tmp_path: Path,
) -> None:
    path = tmp_path / "nested" / "report.json"
    payload = {"z": {"z": "café", "a": [2, 1]}, "a": True}

    _write_json_file(path, payload)

    assert path.read_text(encoding="utf-8") == (
        "{\n"
        '  "a": true,\n'
        '  "z": {\n'
        '    "a": [\n'
        "      2,\n"
        "      1\n"
        "    ],\n"
        '    "z": "café"\n'
        "  }\n"
        "}\n"
    )
    assert json.loads(path.read_text(encoding="utf-8")) == payload


def test_committed_regression_baseline_is_sorted_human_readable_json() -> None:
    baseline = Path(__file__).with_name("regression-baseline-v1.json")
    content = baseline.read_text(encoding="utf-8")

    assert content == (
        json.dumps(json.loads(content), ensure_ascii=False, indent=2, sort_keys=True)
        + "\n"
    )


def test_compact_regression_baselines_are_deterministic(tmp_path: Path) -> None:
    first = tmp_path / "first.json"
    second = tmp_path / "second.json"

    _write_compact_regression_baseline(_regression_report(), first)
    _write_compact_regression_baseline(_regression_report(), second)

    assert first.read_bytes() == second.read_bytes()
    content = first.read_text(encoding="utf-8")
    assert content == (
        json.dumps(json.loads(content), ensure_ascii=False, indent=2, sort_keys=True)
        + "\n"
    )
    assert json.loads(content)["baselineKind"] == (COMPACT_REGRESSION_BASELINE_KIND)


def test_regression_cli_extracts_compact_baseline_without_starting_conformance(
    tmp_path: Path,
) -> None:
    codex = tmp_path / "codex"
    codex.write_text("#!/bin/sh\nexit 99\n", encoding="utf-8")
    codex.chmod(0o755)
    source = tmp_path / "full-report.json"
    source.write_text(json.dumps(_regression_report()), encoding="utf-8")
    extracted = tmp_path / "compact.json"

    assert (
        main(
            [
                str(codex),
                "--baseline-report",
                str(source),
                "--extract-baseline",
                str(extracted),
            ]
        )
        == 0
    )
    assert json.loads(extracted.read_text(encoding="utf-8"))["baselineKind"] == (
        COMPACT_REGRESSION_BASELINE_KIND
    )


def test_regression_cli_rejects_extraction_without_a_source_report(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    codex = tmp_path / "codex"
    codex.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    codex.chmod(0o755)

    assert main([str(codex), "--extract-baseline", str(tmp_path / "compact.json")]) == 2
    assert "requires --baseline-report" in capsys.readouterr().err


@pytest.mark.parametrize("payload", ["{", "[]", "null"])
def test_regression_cli_rejects_malformed_baselines(
    payload: str,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    codex = tmp_path / "codex"
    codex.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    codex.chmod(0o755)
    baseline = tmp_path / "baseline.json"
    baseline.write_text(payload, encoding="utf-8")

    assert main([str(codex), "--baseline-report", str(baseline)]) == 2
    assert "baseline report" in capsys.readouterr().err


def test_regression_cli_rejects_missing_baseline_before_starting_conformance(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    codex = tmp_path / "codex"
    codex.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    codex.chmod(0o755)

    assert main([str(codex), "--baseline-report", str(tmp_path / "missing.json")]) == 2
    assert "cannot read baseline report" in capsys.readouterr().err
