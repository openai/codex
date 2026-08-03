"""Driver for the upstream MCP client conformance suite.

The upstream suite owns the localhost HTTP server and the wire-level checks.
This module deliberately invokes one scenario at a time because the upstream
parallel suite path does not currently propagate the client adapter result.
"""

import json
import os
import re
import shlex
import signal
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Mapping, Sequence

SHIPPING_LEGACY_VERSION = "2025-06-18"
LEGACY_VERSION = "2025-11-25"
MODERN_VERSION = "2026-07-28"

# Reviewed on 2026-07-29. Pinning a commit instead of an npm prerelease keeps
# the scenario definitions and result semantics reproducible.
OFFICIAL_CONFORMANCE_GIT_REF = "49103de6ed70804e940637bf3e9e29e4a3f54e64"
OFFICIAL_CONFORMANCE_REPOSITORY = "modelcontextprotocol/conformance"

OFFICIAL_NON_AUTH_SCENARIOS: Mapping[str, tuple[str, ...]] = {
    SHIPPING_LEGACY_VERSION: (
        "initialize",
        "tools_call",
    ),
    LEGACY_VERSION: (
        "initialize",
        "tools_call",
        "elicitation-sep1034-client-defaults",
        "sse-retry",
    ),
    MODERN_VERSION: (
        "tools_call",
        "request-metadata",
        "sep-2322-client-request-state",
        "http-standard-headers",
        "http-custom-headers",
        "http-invalid-tool-headers",
        "json-schema-ref-no-deref",
    ),
}

OFFICIAL_AUTH_SCENARIOS: Mapping[str, tuple[str, ...]] = {
    SHIPPING_LEGACY_VERSION: (
        "auth/token-endpoint-auth-basic",
        "auth/token-endpoint-auth-post",
        "auth/token-endpoint-auth-none",
    ),
    LEGACY_VERSION: (
        "auth/metadata-default",
        "auth/metadata-var1",
        "auth/metadata-var2",
        "auth/metadata-var3",
        "auth/basic-cimd",
        "auth/scope-from-www-authenticate",
        "auth/scope-from-scopes-supported",
        "auth/scope-omitted-when-undefined",
        "auth/scope-step-up",
        "auth/scope-retry-limit",
        "auth/token-endpoint-auth-basic",
        "auth/token-endpoint-auth-post",
        "auth/token-endpoint-auth-none",
        "auth/pre-registration",
    ),
    MODERN_VERSION: (
        "auth/metadata-default",
        "auth/metadata-var1",
        "auth/metadata-var2",
        "auth/metadata-var3",
        "auth/basic-cimd",
        "auth/scope-from-www-authenticate",
        "auth/scope-from-scopes-supported",
        "auth/scope-omitted-when-undefined",
        "auth/scope-step-up",
        "auth/scope-retry-limit",
        "auth/token-endpoint-auth-basic",
        "auth/token-endpoint-auth-post",
        "auth/token-endpoint-auth-none",
        "auth/pre-registration",
        "auth/resource-mismatch",
        "auth/offline-access-scope",
        "auth/offline-access-not-supported",
        "auth/authorization-server-migration",
        "auth/iss-supported",
        "auth/iss-not-advertised",
        "auth/iss-supported-missing",
        "auth/iss-wrong-issuer",
        "auth/iss-unexpected",
        "auth/iss-normalized",
        "auth/metadata-issuer-mismatch",
    ),
}

# These scenarios cover separately negotiated protocol extensions rather than
# either dated release. Keep them visible for future adapters, but do not mix
# them into versioned release conformance percentages.
OFFICIAL_AUTH_EXTENSION_SCENARIOS: tuple[str, ...] = (
    "auth/client-credentials-jwt",
    "auth/client-credentials-basic",
    "auth/enterprise-managed-authorization",
    "auth/dpop",
    "auth/dpop-nonce",
    "auth/wif-jwt-bearer",
)

_SENSITIVE_JSON_FIELD = re.compile(
    r'("(?:client_secret|private_key_pem|valid_jwt|wrong_audience_jwt|'
    r'expired_jwt|idp_id_token)"\s*:\s*)"(?:\\.|[^"\\])*"'
)
_SENSITIVE_ESCAPED_JSON_FIELD = re.compile(
    r'(\\"(?:client_secret|private_key_pem|valid_jwt|wrong_audience_jwt|'
    r'expired_jwt|idp_id_token)\\"\s*:\s*)\\"(?:\\\\.|[^"\\])*\\"'
)
_SENSITIVE_URL_PARAMETER = re.compile(
    r"([?&](?:code|state|code_challenge|code_verifier|access_token|"
    r"""refresh_token|client_secret)=)[^&\s"'\\<>]+""",
    re.IGNORECASE,
)
_BEARER_TOKEN = re.compile(r"(\bBearer\s+)[A-Za-z0-9._~+/=-]+", re.IGNORECASE)


def redact_sensitive_text(value: str) -> str:
    value = _SENSITIVE_JSON_FIELD.sub(r'\1"[REDACTED]"', value)
    value = _SENSITIVE_ESCAPED_JSON_FIELD.sub(r'\1\\"[REDACTED]\\"', value)
    value = _SENSITIVE_URL_PARAMETER.sub(r"\1[REDACTED]", value)
    return _BEARER_TOKEN.sub(r"\1[REDACTED]", value)


def _scrub_retained_artifacts(scenario_dir: Path) -> None:
    for name in ("stdout.txt", "stderr.txt"):
        for path in scenario_dir.rglob(name):
            try:
                original = path.read_text(encoding="utf-8")
                redacted = redact_sensitive_text(original)
                if redacted != original:
                    path.write_text(redacted, encoding="utf-8")
            except OSError:
                continue
    for path in scenario_dir.rglob(".credentials.json"):
        try:
            path.unlink(missing_ok=True)
        except OSError:
            continue


@dataclass(frozen=True)
class OfficialCheck:
    scenario: str
    check_id: str
    name: str
    status: str
    description: str
    error_message: str | None = None


@dataclass
class OfficialScenarioResult:
    scenario: str
    success: bool
    checks: list[OfficialCheck] = field(default_factory=list)
    adapter_success: bool = False
    adapter_detail: str = ""
    runner_detail: str = ""


def default_conformance_command() -> list[str]:
    return [
        "npx",
        "--yes",
        (f"github:{OFFICIAL_CONFORMANCE_REPOSITORY}#{OFFICIAL_CONFORMANCE_GIT_REF}"),
    ]


def scenarios_for_mode(
    mode: str,
    requested: Sequence[str] | None = None,
    *,
    include_auth: bool = True,
) -> tuple[str, ...]:
    non_auth = OFFICIAL_NON_AUTH_SCENARIOS.get(mode)
    auth = OFFICIAL_AUTH_SCENARIOS.get(mode)
    if non_auth is None or auth is None:
        raise ValueError(f"unsupported MCP protocol version: {mode}")
    available = non_auth + (auth if include_auth else ())
    if not requested:
        return available
    all_scenarios = {
        scenario
        for scenario_map in (
            OFFICIAL_NON_AUTH_SCENARIOS,
            OFFICIAL_AUTH_SCENARIOS,
        )
        for mode_scenarios in scenario_map.values()
        for scenario in mode_scenarios
    }
    unknown = sorted(set(requested) - all_scenarios)
    if unknown:
        raise ValueError(
            "scenarios are not part of the pinned versioned client suite: "
            + ", ".join(unknown)
        )
    disabled = sorted(
        set(requested)
        & {
            scenario
            for mode_scenarios in OFFICIAL_AUTH_SCENARIOS.values()
            for scenario in mode_scenarios
        }
        if not include_auth
        else ()
    )
    if disabled:
        raise ValueError(
            "authentication scenarios require auth coverage to be enabled: "
            + ", ".join(disabled)
        )
    requested_set = set(requested)
    selected = tuple(scenario for scenario in available if scenario in requested_set)
    if not selected:
        raise ValueError(
            f"requested scenarios are unavailable for MCP protocol version {mode}: "
            + ", ".join(sorted(requested_set))
        )
    return selected


def _safe_scenario_name(scenario: str) -> str:
    return re.sub(r"[^A-Za-z0-9_.-]+", "-", scenario)


def _make_adapter_launcher(
    adapter_script: Path,
    *,
    require_automatic_auth: bool = False,
) -> Path:
    # The official runner currently splits --command on spaces before spawning
    # it. Put a one-token launcher in the system temp directory so arbitrary
    # checkout paths (including paths with spaces) remain supported.
    launcher_dir = Path(tempfile.mkdtemp(prefix="mcp-conformance-adapter-"))
    automatic_auth = "1" if require_automatic_auth else "0"
    if sys.platform == "win32":
        launcher = launcher_dir / "client.cmd"
        adapter_command = subprocess.list2cmdline([sys.executable, str(adapter_script)])
        launcher.write_text(
            "@echo off\n"
            f'set "CODEX_CONFORMANCE_REQUIRE_AUTOMATIC_AUTH={automatic_auth}"\n'
            f"{adapter_command} %*\n",
            encoding="utf-8",
        )
    else:
        launcher = launcher_dir / "client"
        launcher.write_text(
            "#!/bin/sh\n"
            f"export CODEX_CONFORMANCE_REQUIRE_AUTOMATIC_AUTH={automatic_auth}\n"
            f'exec {shlex.quote(sys.executable)} {shlex.quote(str(adapter_script))} "$@"\n',
            encoding="utf-8",
        )
    launcher.chmod(0o755)
    return launcher


def _terminate_process_group(
    process: subprocess.Popen[str],
    *,
    force: bool = False,
) -> None:
    if sys.platform == "win32":
        command = ["taskkill", "/T", "/PID", str(process.pid)]
        if force:
            command.insert(1, "/F")
        try:
            result = subprocess.run(
                command,
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            if result.returncode == 0:
                return
        except OSError:
            pass
        try:
            if force:
                process.kill()
            else:
                process.terminate()
        except (OSError, ProcessLookupError):
            pass
        return

    try:
        os.killpg(process.pid, signal.SIGKILL if force else signal.SIGTERM)
    except (OSError, ProcessLookupError):
        pass


def _load_checks(result_dir: Path, scenario: str) -> list[OfficialCheck]:
    check_files = sorted(result_dir.rglob("checks.json"))
    if len(check_files) != 1:
        return []
    try:
        decoded = json.loads(check_files[0].read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []
    if not isinstance(decoded, list):
        return []

    checks: list[OfficialCheck] = []
    for index, raw in enumerate(decoded, start=1):
        if not isinstance(raw, dict):
            continue
        check_id = str(raw.get("id") or f"unnamed-{index}")
        name = str(raw.get("name") or check_id)
        status = str(raw.get("status") or "FAILURE").upper()
        # INFO entries are the suite's HTTP trace, not conformance assertions.
        # They remain available in the retained upstream checks.json artifact.
        if status == "INFO":
            continue
        checks.append(
            OfficialCheck(
                scenario=scenario,
                check_id=check_id,
                name=name,
                status=status,
                description=str(raw.get("description") or name),
                error_message=(
                    redact_sensitive_text(str(raw["errorMessage"]))
                    if raw.get("errorMessage") is not None
                    else None
                ),
            )
        )
    return checks


def _load_adapter_report(report_path: Path) -> tuple[bool, str]:
    try:
        decoded = json.loads(report_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return False, f"Codex adapter did not write a valid report: {exc}"
    if not isinstance(decoded, dict):
        return False, "Codex adapter report was not an object"
    success = decoded.get("success") is True
    steps = decoded.get("steps")
    failed_steps: list[str] = []
    if isinstance(steps, list):
        for step in steps:
            if isinstance(step, dict) and step.get("success") is not True:
                failed_steps.append(
                    f"{step.get('name')}: {step.get('detail', 'failed')}"
                )
    if success:
        return True, "Codex adapter completed the scenario"
    if failed_steps:
        return False, redact_sensitive_text("; ".join(failed_steps))[-4_000:]
    return False, redact_sensitive_text(
        str(decoded.get("error") or "Codex adapter failed")
    )[-4_000:]


def _run_scenario(
    *,
    conformance_command: Sequence[str],
    adapter_launcher: Path,
    codex_binary: Path,
    mode: str,
    scenario: str,
    output_dir: Path,
    timeout_seconds: float,
    base_env: Mapping[str, str],
    process_grace_seconds: float = 15,
) -> OfficialScenarioResult:
    scenario_dir = output_dir / _safe_scenario_name(scenario)
    scenario_dir.mkdir(parents=True, exist_ok=True)
    adapter_report = scenario_dir / "codex-adapter.json"
    adapter_home = scenario_dir / "codex-home"
    adapter_home.mkdir()

    env = dict(base_env)
    env.update(
        {
            "CODEX_CONFORMANCE_BINARY": str(codex_binary),
            "CODEX_CONFORMANCE_HOME": str(adapter_home),
            "CODEX_CONFORMANCE_ADAPTER_REPORT": str(adapter_report),
        }
    )
    command = [
        *conformance_command,
        "client",
        "--command",
        str(adapter_launcher),
        "--scenario",
        scenario,
        "--spec-version",
        mode,
        "--timeout",
        str(round(timeout_seconds * 1_000)),
        "--output-dir",
        str(scenario_dir),
    ]
    process: subprocess.Popen[str] | None = None
    try:
        process = subprocess.Popen(
            command,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=sys.platform != "win32",
            creationflags=(
                getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
                if sys.platform == "win32"
                else 0
            ),
        )
        try:
            process_timeout = timeout_seconds + process_grace_seconds
            stdout, stderr = process.communicate(timeout=process_timeout)
            runner_detail = redact_sensitive_text((stderr or stdout).strip())[-8_000:]
            returncode = process.returncode
        except subprocess.TimeoutExpired:
            _terminate_process_group(process)
            try:
                stdout, stderr = process.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                _terminate_process_group(process, force=True)
                stdout, stderr = process.communicate()
            runner_detail = redact_sensitive_text(
                (stderr or stdout)
                + f"\nofficial scenario timed out after {process_timeout:g}s"
            ).strip()[-8_000:]
            returncode = 124
    except OSError as exc:
        runner_detail = redact_sensitive_text(str(exc))
        returncode = 127

    # A crashing upstream runner can exit before its still-running adapter has
    # atomically written the diagnostic report. Give that report a short grace
    # period so an upstream fixture crash is not mislabeled as an adapter
    # failure, then terminate any orphaned descendants in the runner's process
    # group.
    if returncode != 0 and not adapter_report.is_file():
        report_deadline = time.monotonic() + min(process_grace_seconds, 10)
        while time.monotonic() < report_deadline and not adapter_report.is_file():
            time.sleep(0.05)
    if returncode != 0 and process is not None:
        _terminate_process_group(process)

    _scrub_retained_artifacts(scenario_dir)
    checks = _load_checks(scenario_dir, scenario)
    adapter_success, adapter_detail = _load_adapter_report(adapter_report)
    official_failure = any(check.status in {"FAILURE", "WARNING"} for check in checks)
    success = (
        returncode == 0 and bool(checks) and adapter_success and not official_failure
    )
    if not checks:
        runner_detail = (
            "official runner did not produce exactly one checks.json; " + runner_detail
        ).strip()
    elif returncode != 0 and not official_failure and adapter_success:
        runner_detail = (
            f"official runner exited with code {returncode}; " + runner_detail
        ).strip()
    return OfficialScenarioResult(
        scenario=scenario,
        success=success,
        checks=checks,
        adapter_success=adapter_success,
        adapter_detail=adapter_detail,
        runner_detail=runner_detail,
    )


def run_official_mode(
    *,
    conformance_command: Sequence[str],
    adapter_script: Path,
    codex_binary: Path,
    mode: str,
    scenarios: Sequence[str],
    output_dir: Path,
    timeout_seconds: float,
    base_env: Mapping[str, str],
) -> list[OfficialScenarioResult]:
    launcher = _make_adapter_launcher(
        adapter_script,
        require_automatic_auth=base_env.get("CODEX_CONFORMANCE_REQUIRE_AUTOMATIC_AUTH")
        == "1",
    )
    try:
        return [
            _run_scenario(
                conformance_command=conformance_command,
                adapter_launcher=launcher,
                codex_binary=codex_binary,
                mode=mode,
                scenario=scenario,
                output_dir=output_dir,
                timeout_seconds=timeout_seconds,
                base_env=base_env,
            )
            for scenario in scenarios
        ]
    finally:
        try:
            launcher.unlink()
            launcher.parent.rmdir()
        except OSError:
            pass
