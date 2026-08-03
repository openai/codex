import json
import subprocess
import sys
import urllib.parse
from pathlib import Path

import codex_conformance_adapter
import official_conformance
import pytest
import run_codex_compliance
from codex_conformance_adapter import (
    CIMD_CLIENT_METADATA_URL,
    AdapterFailure,
    _exercise_auth_scenario,
    _oauth_client_id,
    _validate_oauth_secret_not_persisted,
    _validated_callback_url,
    _write_auth_registration,
)
from official_conformance import (
    LEGACY_VERSION,
    MODERN_VERSION,
    OFFICIAL_CONFORMANCE_GIT_REF,
    SHIPPING_LEGACY_VERSION,
    _make_adapter_launcher,
    _run_scenario,
    _scrub_retained_artifacts,
    _terminate_process_group,
    default_conformance_command,
    redact_sensitive_text,
    run_official_mode,
    scenarios_for_mode,
)


def _authorization_url(redirect_uri: str, state: str = "test-state") -> str:
    return "http://127.0.0.1:8765/authorize?" + urllib.parse.urlencode(
        {
            "client_id": "test-client",
            "redirect_uri": redirect_uri,
            "state": state,
        }
    )


def test_auth_adapter_only_accepts_exact_loopback_callback() -> None:
    redirect_uri = "http://127.0.0.1:32123/callback"
    authorization_url = _authorization_url(redirect_uri)
    callback_url = redirect_uri + "?code=test-code&state=test-state"

    assert _validated_callback_url(authorization_url, callback_url) == callback_url

    with pytest.raises(AdapterFailure, match="exact loopback"):
        _validated_callback_url(
            authorization_url,
            "http://127.0.0.1:32124/callback?code=test-code&state=test-state",
        )
    with pytest.raises(AdapterFailure, match="preserve OAuth state"):
        _validated_callback_url(
            authorization_url,
            redirect_uri + "?code=test-code&state=wrong-state",
        )
    with pytest.raises(AdapterFailure, match="safe loopback"):
        _validated_callback_url(
            _authorization_url("https://example.com/callback"),
            "https://example.com/callback?code=test-code&state=test-state",
        )


@pytest.mark.parametrize(
    ("query", "message"),
    [
        ("code=first&code=second&state=test-state", "exactly one nonempty code"),
        ("code=&state=test-state", "exactly one nonempty code"),
        ("error=first&error=second&state=test-state", "exactly one nonempty error"),
        ("error=&state=test-state", "exactly one nonempty error"),
        ("code=test-code&error=access_denied&state=test-state", "exactly one of"),
        ("code=test-code&state=test-state&state=test-state", "preserve OAuth state"),
        (
            "code=test-code&state=test-state&iss=first&iss=second",
            "exactly one nonempty iss",
        ),
        ("code=test-code&state=test-state&iss=", "exactly one nonempty iss"),
        ("state=test-state", "exactly one of"),
    ],
)
def test_auth_adapter_rejects_ambiguous_oauth_callback_parameters(
    query: str,
    message: str,
) -> None:
    redirect_uri = "http://127.0.0.1:32123/callback"

    with pytest.raises(AdapterFailure, match=message):
        _validated_callback_url(
            _authorization_url(redirect_uri),
            f"{redirect_uri}?{query}",
        )


def test_auth_adapter_allows_a_single_error_callback_for_client_validation() -> None:
    redirect_uri = "http://127.0.0.1:32123/callback"
    callback_url = redirect_uri + "?error=access_denied&state=test-state&iss=issuer"

    assert (
        _validated_callback_url(_authorization_url(redirect_uri), callback_url)
        == callback_url
    )


def test_official_runner_is_pinned_instead_of_using_npm_latest() -> None:
    command = default_conformance_command()

    assert OFFICIAL_CONFORMANCE_GIT_REF in command[-1]
    assert "@latest" not in command[-1]


def test_official_adapter_launcher_preserves_strict_production_auth_mode(
    tmp_path: Path,
) -> None:
    adapter = tmp_path / "show_automatic_auth.py"
    adapter.write_text(
        'import os\nprint(os.environ.get("CODEX_CONFORMANCE_REQUIRE_AUTOMATIC_AUTH", ""))\n',
        encoding="utf-8",
    )
    launcher = _make_adapter_launcher(adapter, require_automatic_auth=True)

    try:
        result = subprocess.run(
            [str(launcher)],
            capture_output=True,
            check=True,
            text=True,
        )
    finally:
        launcher.unlink()
        launcher.parent.rmdir()

    assert result.stdout.strip() == "1"


@pytest.mark.parametrize("require_automatic_auth", [False, True])
def test_windows_official_adapter_launcher_quotes_checkout_paths(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    require_automatic_auth: bool,
) -> None:
    adapter = tmp_path / "checkout with spaces" / "adapter script.py"
    adapter.parent.mkdir()
    adapter.write_text("raise AssertionError('launcher generation only')\n")
    monkeypatch.setattr(official_conformance.sys, "platform", "win32")

    launcher = _make_adapter_launcher(
        adapter,
        require_automatic_auth=require_automatic_auth,
    )
    try:
        contents = launcher.read_text(encoding="utf-8")
    finally:
        launcher.unlink()
        launcher.parent.rmdir()

    expected_auth = "1" if require_automatic_auth else "0"
    assert launcher.name == "client.cmd"
    assert contents.startswith("@echo off\n")
    assert (
        f'set "CODEX_CONFORMANCE_REQUIRE_AUTOMATIC_AUTH={expected_auth}"\n' in contents
    )
    assert subprocess.list2cmdline([sys.executable, str(adapter)]) + " %*\n" in contents


@pytest.mark.parametrize(
    ("force", "expected_command"),
    [
        (False, ["taskkill", "/T", "/PID", "417"]),
        (True, ["taskkill", "/F", "/T", "/PID", "417"]),
    ],
)
def test_windows_official_timeout_terminates_the_entire_process_tree(
    monkeypatch: pytest.MonkeyPatch,
    force: bool,
    expected_command: list[str],
) -> None:
    observed: list[list[str]] = []

    class FakeProcess:
        pid = 417

        def terminate(self) -> None:
            raise AssertionError("successful taskkill must own tree termination")

        def kill(self) -> None:
            raise AssertionError("successful taskkill must own tree termination")

    def fake_run(
        command: list[str],
        *,
        check: bool,
        stdout: int,
        stderr: int,
    ) -> subprocess.CompletedProcess[str]:
        observed.append(command)
        assert check is False
        assert stdout == subprocess.DEVNULL
        assert stderr == subprocess.DEVNULL
        return subprocess.CompletedProcess(command, 0)

    monkeypatch.setattr(official_conformance.sys, "platform", "win32")
    monkeypatch.setattr(official_conformance.subprocess, "run", fake_run)

    _terminate_process_group(
        FakeProcess(),  # type: ignore[arg-type] - validate the Popen process contract.
        force=force,
    )

    assert observed == [expected_command]


@pytest.mark.parametrize(
    "scenario",
    ["auth/scope-step-up", "auth/authorization-server-migration"],
)
def test_strict_auth_does_not_inject_a_second_login(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    scenario: str,
) -> None:
    manual_reauthorizations: list[object] = []

    monkeypatch.setattr(
        codex_conformance_adapter,
        "_oauth_login",
        lambda _client, **_kwargs: (True, None),
    )
    monkeypatch.setattr(codex_conformance_adapter, "_reload_mcp", lambda _client: None)
    monkeypatch.setattr(
        codex_conformance_adapter, "_auth_inventory", lambda _client: {}
    )

    def fail_tool_call(_client: object, _workspace: Path) -> None:
        raise AdapterFailure("reauthorization is required")

    monkeypatch.setattr(codex_conformance_adapter, "_auth_tool_call", fail_tool_call)
    monkeypatch.setattr(
        codex_conformance_adapter,
        "_login_reload_and_call",
        lambda *args, **kwargs: manual_reauthorizations.append((args, kwargs)),
    )

    with pytest.raises(AdapterFailure, match="reauthorization is required"):
        _exercise_auth_scenario(
            object(),  # type: ignore[arg-type]
            scenario=scenario,
            workspace=tmp_path,
            timeout_seconds=1,
            require_automatic_auth=True,
        )

    assert manual_reauthorizations == []


@pytest.mark.parametrize(
    "scenario",
    ["auth/scope-step-up", "auth/authorization-server-migration"],
)
def test_strict_auth_accepts_product_owned_reauthentication(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    scenario: str,
) -> None:
    tool_calls: list[Path] = []
    manual_reauthorizations: list[object] = []

    monkeypatch.setattr(
        codex_conformance_adapter,
        "_oauth_login",
        lambda _client, **_kwargs: (True, None),
    )
    monkeypatch.setattr(codex_conformance_adapter, "_reload_mcp", lambda _client: None)
    monkeypatch.setattr(
        codex_conformance_adapter, "_auth_inventory", lambda _client: {}
    )
    monkeypatch.setattr(
        codex_conformance_adapter,
        "_auth_tool_call",
        lambda _client, workspace: tool_calls.append(workspace),
    )
    monkeypatch.setattr(
        codex_conformance_adapter,
        "_login_reload_and_call",
        lambda *args, **kwargs: manual_reauthorizations.append((args, kwargs)),
    )

    detail = _exercise_auth_scenario(
        object(),  # type: ignore[arg-type]
        scenario=scenario,
        workspace=tmp_path,
        timeout_seconds=1,
        require_automatic_auth=True,
    )

    assert "automatically" in detail
    assert tool_calls == [tmp_path]
    assert manual_reauthorizations == []


def test_strict_auth_does_not_invent_a_production_client_metadata_url() -> None:
    assert (
        _oauth_client_id(
            "auth/basic-cimd",
            {},
            require_production_client_identity=True,
        )
        is None
    )


@pytest.mark.parametrize("filename", ["config.toml", ".credentials.json"])
def test_oauth_client_secret_persistence_is_detected_without_disclosing_it(
    tmp_path: Path,
    filename: str,
) -> None:
    secret = "reviewer-confidential-client-secret"
    (tmp_path / filename).write_text(
        json.dumps({"client_secret": secret}),
        encoding="utf-8",
    )

    with pytest.raises(AdapterFailure, match="client secret was persisted") as error:
        _validate_oauth_secret_not_persisted(tmp_path, secret)

    assert filename in str(error.value)
    assert secret not in str(error.value)


def test_oauth_client_secret_environment_reference_is_not_a_persisted_secret(
    tmp_path: Path,
) -> None:
    (tmp_path / "config.toml").write_text(
        'client_secret_env_var = "MCP_CONFORMANCE_CLIENT_SECRET"\n',
        encoding="utf-8",
    )

    _validate_oauth_secret_not_persisted(
        tmp_path, "reviewer-confidential-client-secret"
    )


def test_auth_adapter_selects_only_scenario_provided_client_ids() -> None:
    assert _oauth_client_id("auth/basic-cimd", {}) == CIMD_CLIENT_METADATA_URL
    assert (
        _oauth_client_id(
            "auth/basic-cimd",
            {},
            require_production_client_identity=True,
        )
        is None
    )
    assert (
        _oauth_client_id(
            "auth/pre-registration",
            {"client_id": "pre-registered", "client_secret": "do-not-log"},
        )
        == "pre-registered"
    )
    assert _oauth_client_id("auth/metadata-default", {}) is None
    with pytest.raises(AdapterFailure, match="client_id"):
        _oauth_client_id("auth/pre-registration", {})


def test_auth_registration_does_not_persist_context_secret(tmp_path: Path) -> None:
    config_path = tmp_path / "config.toml"
    config_path.write_text('mcp_oauth_credentials_store = "file"\n', encoding="utf-8")

    _write_auth_registration(
        config_path,
        server_url="http://127.0.0.1:8765/mcp",
        oauth_client_id="pre-registered-client",
        oauth_client_secret_env_var="MCP_CONFORMANCE_CLIENT_SECRET",
    )

    config = config_path.read_text(encoding="utf-8")
    assert 'url = "http://127.0.0.1:8765/mcp"' in config
    assert 'client_id = "pre-registered-client"' in config
    assert 'client_secret_env_var = "MCP_CONFORMANCE_CLIENT_SECRET"' in config
    assert "do-not-log" not in config
    assert "client_secret =" not in config


def test_official_diagnostics_redact_oauth_secrets() -> None:
    diagnostic = (
        'With context: {"client_id":"visible","client_secret":"secret-value",'
        '"private_key_pem":"private-value"}\n'
        "Authorize at http://localhost/authorize?state=state-value&code_challenge=pkce-value\n"
        "Authorization: Bearer token-value"
    )

    redacted = redact_sensitive_text(diagnostic)

    assert '"client_id":"visible"' in redacted
    assert '"client_secret":"[REDACTED]"' in redacted
    assert '"private_key_pem":"[REDACTED]"' in redacted
    assert "state=[REDACTED]" in redacted
    assert "code_challenge=[REDACTED]" in redacted
    assert "Bearer [REDACTED]" in redacted
    serialized = redact_sensitive_text(json.dumps({"diagnostic": diagnostic}))
    assert json.loads(serialized)["diagnostic"]
    for secret in (
        "secret-value",
        "private-value",
        "state-value",
        "pkce-value",
        "token-value",
    ):
        assert secret not in redacted
        assert secret not in serialized


def test_retained_artifacts_remove_oauth_credential_store(tmp_path: Path) -> None:
    codex_home = tmp_path / "codex-home"
    codex_home.mkdir()
    credentials = codex_home / ".credentials.json"
    credentials.write_text('{"client_secret":"do-not-retain"}', encoding="utf-8")
    stdout = tmp_path / "stdout.txt"
    stdout.write_text("Bearer token-value", encoding="utf-8")

    _scrub_retained_artifacts(tmp_path)

    assert not credentials.exists()
    assert stdout.read_text(encoding="utf-8") == "Bearer [REDACTED]"


def test_pinned_full_scenarios_are_selected_by_protocol_version() -> None:
    shipping_legacy = scenarios_for_mode(SHIPPING_LEGACY_VERSION)
    legacy = scenarios_for_mode(LEGACY_VERSION)
    modern = scenarios_for_mode(MODERN_VERSION)

    assert shipping_legacy == (
        "initialize",
        "tools_call",
        "auth/token-endpoint-auth-basic",
        "auth/token-endpoint-auth-post",
        "auth/token-endpoint-auth-none",
    )
    assert scenarios_for_mode(SHIPPING_LEGACY_VERSION, include_auth=False) == (
        "initialize",
        "tools_call",
    )

    assert legacy[:4] == (
        "initialize",
        "tools_call",
        "elicitation-sep1034-client-defaults",
        "sse-retry",
    )
    assert len(legacy) == 18
    assert len([scenario for scenario in legacy if scenario.startswith("auth/")]) == 14
    assert "auth/pre-registration" in legacy

    assert modern[:7] == (
        "tools_call",
        "request-metadata",
        "sep-2322-client-request-state",
        "http-standard-headers",
        "http-custom-headers",
        "http-invalid-tool-headers",
        "json-schema-ref-no-deref",
    )
    assert len(modern) == 32
    assert len([scenario for scenario in modern if scenario.startswith("auth/")]) == 25
    assert "auth/resource-mismatch" in modern
    assert "auth/metadata-issuer-mismatch" in modern

    assert scenarios_for_mode(LEGACY_VERSION, include_auth=False) == legacy[:4]
    assert scenarios_for_mode(MODERN_VERSION, include_auth=False) == modern[:7]
    assert scenarios_for_mode(
        MODERN_VERSION,
        ["tools_call", "http-custom-headers"],
    ) == ("tools_call", "http-custom-headers")
    assert len(OFFICIAL_CONFORMANCE_GIT_REF) == 40


@pytest.mark.parametrize(
    ("mode", "scenario"),
    [
        (SHIPPING_LEGACY_VERSION, "request-metadata"),
        (LEGACY_VERSION, "http-standard-headers"),
        (MODERN_VERSION, "elicitation-sep1034-client-defaults"),
    ],
)
def test_versioned_scenario_selection_rejects_an_incompatible_protocol(
    mode: str, scenario: str
) -> None:
    with pytest.raises(
        ValueError,
        match=rf"unavailable for MCP protocol version {mode}.*{scenario}",
    ):
        scenarios_for_mode(mode, [scenario], include_auth=False)


def test_cross_version_auth_subset_remains_available_to_every_mode() -> None:
    requested = (
        "auth/token-endpoint-auth-basic",
        "auth/resource-mismatch",
        "auth/authorization-server-migration",
    )

    assert scenarios_for_mode(SHIPPING_LEGACY_VERSION, requested) == (
        "auth/token-endpoint-auth-basic",
    )
    assert scenarios_for_mode(LEGACY_VERSION, requested) == (
        "auth/token-endpoint-auth-basic",
    )
    assert scenarios_for_mode(MODERN_VERSION, requested) == requested


def test_official_cli_rejects_an_empty_versioned_scenario_run(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    def unexpected_run(*_args: object, **_kwargs: object) -> None:
        raise AssertionError("an incompatible selection must not execute conformance")

    monkeypatch.setattr(run_codex_compliance, "run_compliance", unexpected_run)

    exit_code = run_codex_compliance.main(
        [
            sys.executable,
            "--mode",
            SHIPPING_LEGACY_VERSION,
            "--transport",
            "http",
            "--official-scenario",
            "request-metadata",
            "--no-auth",
            "--conformance-cli",
            sys.executable,
        ]
    )

    assert exit_code == 2
    assert (
        "requested scenarios are unavailable for MCP protocol version 2025-06-18: "
        "request-metadata"
    ) in capsys.readouterr().err


def test_official_driver_reads_checks_and_adapter_report(tmp_path: Path) -> None:
    fake_cli = tmp_path / "fake_conformance.py"
    fake_cli.write_text(
        """
import json
import os
import pathlib
import sys

args = sys.argv[1:]
scenario = args[args.index("--scenario") + 1]
output = pathlib.Path(args[args.index("--output-dir") + 1])
result_dir = output / f"{scenario}-timestamp"
result_dir.mkdir(parents=True)
(result_dir / "checks.json").write_text(json.dumps([
    {
        "id": "request-trace",
        "name": "RequestTrace",
        "description": "not a conformance assertion",
        "status": "INFO",
    },
    {
        "id": "official-check",
        "name": "OfficialCheck",
        "description": "observed",
        "status": "SUCCESS",
    },
]))
pathlib.Path(os.environ["CODEX_CONFORMANCE_ADAPTER_REPORT"]).write_text(
    json.dumps({"success": True, "steps": []})
)
""".lstrip(),
        encoding="utf-8",
    )
    adapter = tmp_path / "adapter.py"
    adapter.write_text("raise AssertionError('fake CLI should not run adapter')\n")

    results = run_official_mode(
        conformance_command=[sys.executable, str(fake_cli)],
        adapter_script=adapter,
        codex_binary=Path("/opt/codex"),
        mode=MODERN_VERSION,
        scenarios=["tools_call"],
        output_dir=tmp_path / "results",
        timeout_seconds=1,
        base_env={},
    )

    assert len(results) == 1
    assert results[0].success
    assert results[0].adapter_success
    assert len(results[0].checks) == 1
    assert results[0].checks[0].check_id == "official-check"


def test_official_driver_terminates_timed_out_process_group(tmp_path: Path) -> None:
    fake_cli = tmp_path / "hanging_conformance.py"
    fake_cli.write_text(
        """
import subprocess
import sys
import time

subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
time.sleep(60)
""".lstrip(),
        encoding="utf-8",
    )
    adapter = tmp_path / "adapter.py"
    adapter.write_text("raise AssertionError('not reached')\n", encoding="utf-8")
    launcher = _make_adapter_launcher(adapter)
    try:
        result = _run_scenario(
            conformance_command=[sys.executable, str(fake_cli)],
            adapter_launcher=launcher,
            codex_binary=Path("/opt/codex"),
            mode=MODERN_VERSION,
            scenario="tools_call",
            output_dir=tmp_path / "results",
            timeout_seconds=0.05,
            process_grace_seconds=0.05,
            base_env={},
        )
    finally:
        launcher.unlink()
        launcher.parent.rmdir()

    assert not result.success
    assert "timed out" in result.runner_detail


def test_official_driver_does_not_mislabel_runner_crash_as_adapter_failure(
    tmp_path: Path,
) -> None:
    fake_cli = tmp_path / "crashing_conformance.py"
    fake_cli.write_text(
        """
import os
import subprocess
import sys

script = '''
import json
import os
import pathlib
import time

time.sleep(0.1)
pathlib.Path(os.environ["CODEX_CONFORMANCE_ADAPTER_REPORT"]).write_text(
    json.dumps({"success": True, "steps": []})
)
'''
subprocess.Popen([sys.executable, "-c", script], env=os.environ)
sys.exit(1)
""".lstrip(),
        encoding="utf-8",
    )
    adapter = tmp_path / "adapter.py"
    adapter.write_text("raise AssertionError('not reached')\n", encoding="utf-8")
    launcher = _make_adapter_launcher(adapter)
    try:
        result = _run_scenario(
            conformance_command=[sys.executable, str(fake_cli)],
            adapter_launcher=launcher,
            codex_binary=Path("/opt/codex"),
            mode=MODERN_VERSION,
            scenario="request-metadata",
            output_dir=tmp_path / "results",
            timeout_seconds=1,
            process_grace_seconds=1,
            base_env={},
        )
    finally:
        launcher.unlink()
        launcher.parent.rmdir()

    assert not result.success
    assert result.adapter_success
    assert "did not produce exactly one checks.json" in result.runner_detail
