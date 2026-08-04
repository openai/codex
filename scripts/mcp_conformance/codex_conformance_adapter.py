#!/usr/bin/env python3
"""Adapt one official MCP client scenario to Codex app-server requests."""

import ipaddress
import json
import os
import sys
import traceback
import urllib.error
import urllib.parse
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Mapping, Sequence
from urllib.request import HTTPRedirectHandler, ProxyHandler, Request, build_opener  # noqa: TID251

_MODULE_DIR = Path(__file__).resolve().parent
if str(_MODULE_DIR) not in sys.path:
    sys.path.insert(0, str(_MODULE_DIR))

from run_codex_compliance import (  # noqa: E402 - direct scripts must first add their sibling directory.
    MODERN_VERSION,
    TEST_SERVER_NAME,
    AppServerClient,
    AppServerError,
    _call_tool,
    _command_detail,
    _isolated_environment,
    _response_result,
    _run_command,
)


@dataclass
class Step:
    name: str
    success: bool
    detail: str


class AdapterFailure(RuntimeError):
    pass


CIMD_CLIENT_METADATA_URL = "https://conformance-test.local/client-metadata.json"
PRE_REGISTERED_CLIENT_SECRET_ENV_VAR = "MCP_CONFORMANCE_CLIENT_SECRET"
AUTH_COMPLETION_METHOD = "mcpServer/oauthLogin/completed"
EXPECTED_AUTH_REJECTION_SCENARIOS = frozenset(
    {
        "auth/resource-mismatch",
        "auth/iss-supported-missing",
        "auth/iss-wrong-issuer",
        "auth/iss-unexpected",
        "auth/iss-normalized",
        "auth/metadata-issuer-mismatch",
    }
)


def _required_path(name: str) -> Path:
    value = os.environ.get(name)
    if not value:
        raise AdapterFailure(f"{name} is required")
    return Path(value).expanduser().resolve()


def _required_env(name: str) -> str:
    value = os.environ.get(name)
    if not value:
        raise AdapterFailure(f"{name} is required")
    return value


def _conformance_context() -> dict[str, object]:
    raw = os.environ.get("MCP_CONFORMANCE_CONTEXT")
    if not raw:
        return {}
    try:
        decoded = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise AdapterFailure(f"invalid MCP_CONFORMANCE_CONTEXT: {exc}") from exc
    if not isinstance(decoded, dict):
        raise AdapterFailure("MCP_CONFORMANCE_CONTEXT was not an object")
    return decoded


def _result_or_raise(
    response: Mapping[str, object],
    operation: str,
) -> dict[str, object]:
    result, detail = _response_result(response)
    if result is None:
        raise AdapterFailure(f"{operation}: {detail}")
    return result


def _server_entry(inventory: Mapping[str, object]) -> dict[str, object]:
    entries = inventory.get("data")
    if not isinstance(entries, list):
        raise AdapterFailure("mcpServerStatus/list returned no data array")
    for entry in entries:
        if isinstance(entry, dict) and entry.get("name") == TEST_SERVER_NAME:
            return entry
    raise AdapterFailure(f"{TEST_SERVER_NAME!r} was absent from MCP status")


def _thread_id(client: AppServerClient, workspace: Path) -> str:
    result = _result_or_raise(
        client.request(
            "thread/start",
            {"cwd": str(workspace), "ephemeral": True},
        ),
        "thread/start",
    )
    thread = result.get("thread")
    if not isinstance(thread, dict) or not isinstance(thread.get("id"), str):
        raise AdapterFailure("thread/start did not return a thread id")
    return str(thread["id"])


def _call(
    client: AppServerClient,
    *,
    thread_id: str,
    tool: str,
    arguments: Mapping[str, object],
) -> None:
    result, detail = _call_tool(
        client,
        thread_id=thread_id,
        tool=tool,
        arguments=arguments,
    )
    if result is None:
        raise AdapterFailure(f"tool {tool}: {detail}")


def _elicitation_content(
    scenario: str,
    _params: Mapping[str, object],
) -> Mapping[str, object]:
    if scenario == "elicitation-sep1034-client-defaults":
        # The official scenario deliberately supplies no values. Codex, as the
        # MCP client under test, must materialize the JSON Schema defaults.
        return {}
    if scenario == "sep-2322-client-request-state":
        return {"confirmed": True}
    return {"confirmation": "confirmed"}


def _context_tool_calls() -> list[tuple[str, dict[str, object]]]:
    context = _conformance_context()
    if not context:
        raise AdapterFailure(
            "official scenario did not provide MCP_CONFORMANCE_CONTEXT"
        )
    calls = context.get("toolCalls")
    if not isinstance(calls, list):
        raise AdapterFailure("official scenario context did not contain toolCalls")
    result: list[tuple[str, dict[str, object]]] = []
    for call in calls:
        if (
            not isinstance(call, dict)
            or not isinstance(call.get("name"), str)
            or not isinstance(call.get("arguments"), dict)
        ):
            raise AdapterFailure("official scenario contained an invalid tool call")
        result.append((str(call["name"]), dict(call["arguments"])))
    return result


def _is_loopback_hostname(hostname: str | None) -> bool:
    if hostname is None:
        return False
    if hostname.lower() == "localhost":
        return True
    try:
        return ipaddress.ip_address(hostname).is_loopback
    except ValueError:
        return False


def _validated_callback_url(authorization_url: str, location: str) -> str:
    authorization = urllib.parse.urlsplit(authorization_url)
    query = urllib.parse.parse_qs(authorization.query, keep_blank_values=True)
    redirect_values = query.get("redirect_uri")
    state_values = query.get("state")
    if not redirect_values or len(redirect_values) != 1:
        raise AdapterFailure(
            "authorization request did not contain exactly one redirect_uri"
        )
    if not state_values or len(state_values) != 1:
        raise AdapterFailure("authorization request did not contain exactly one state")

    redirect = urllib.parse.urlsplit(redirect_values[0])
    if (
        redirect.scheme != "http"
        or not _is_loopback_hostname(redirect.hostname)
        or redirect.username is not None
        or redirect.password is not None
        or redirect.port is None
        or redirect.query
        or redirect.fragment
    ):
        raise AdapterFailure("OAuth redirect_uri was not a safe loopback HTTP endpoint")

    callback_url = urllib.parse.urljoin(authorization_url, location)
    callback = urllib.parse.urlsplit(callback_url)
    expected_endpoint = (
        redirect.scheme,
        redirect.hostname,
        redirect.port,
        redirect.path,
    )
    actual_endpoint = (
        callback.scheme,
        callback.hostname,
        callback.port,
        callback.path,
    )
    if (
        actual_endpoint != expected_endpoint
        or callback.username is not None
        or callback.password is not None
        or callback.fragment
    ):
        raise AdapterFailure(
            "authorization server redirect did not target Codex's exact loopback callback"
        )

    callback_query = urllib.parse.parse_qs(callback.query, keep_blank_values=True)
    callback_states = callback_query.get("state")
    if callback_states != state_values:
        raise AdapterFailure(
            "authorization server redirect did not preserve OAuth state"
        )
    for parameter in ("code", "error", "iss"):
        values = callback_query.get(parameter)
        if values is not None and (len(values) != 1 or not values[0]):
            raise AdapterFailure(
                f"authorization server redirect must contain exactly one nonempty {parameter}"
            )
    if ("code" in callback_query) == ("error" in callback_query):
        raise AdapterFailure(
            "authorization server redirect must contain exactly one of code or error"
        )
    return callback_url


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(
        self,
        req: Request,
        fp: object,
        code: int,
        msg: str,
        headers: Mapping[str, str],
        newurl: str,
    ) -> Request | None:
        del req, fp, code, msg, headers, newurl
        return None


def _open_without_redirects(url: str, timeout_seconds: float) -> tuple[int, str | None]:
    opener = build_opener(
        ProxyHandler({}),
        _NoRedirect(),
    )
    request = Request(
        url,
        method="GET",
        headers={"Accept": "text/html,application/xhtml+xml"},
    )
    try:
        with opener.open(request, timeout=timeout_seconds) as response:
            return response.status, response.headers.get("Location")
    except urllib.error.HTTPError as exc:
        # With redirects disabled urllib represents 3xx as HTTPError. Closing
        # the body promptly prevents an authorization page from being retained.
        try:
            return exc.code, exc.headers.get("Location")
        finally:
            exc.close()


def _drive_headless_authorization(
    authorization_url: str,
    *,
    timeout_seconds: float,
) -> None:
    status, location = _open_without_redirects(authorization_url, timeout_seconds)
    if status not in {301, 302, 303, 307, 308} or not location:
        raise AdapterFailure(
            "authorization endpoint did not issue the expected callback redirect"
        )
    callback_url = _validated_callback_url(authorization_url, location)
    callback_status, _ = _open_without_redirects(callback_url, timeout_seconds)
    # Error callbacks may intentionally return a 4xx after notifying the OAuth
    # waiter. The completion notification is the authoritative outcome.
    if not 200 <= callback_status < 500:
        raise AdapterFailure("Codex OAuth callback endpoint returned an invalid status")


def _oauth_client_id(
    scenario: str,
    context: Mapping[str, object],
    *,
    require_production_client_identity: bool = False,
) -> str | None:
    if scenario == "auth/basic-cimd":
        return None if require_production_client_identity else CIMD_CLIENT_METADATA_URL
    if scenario == "auth/pre-registration":
        client_id = context.get("client_id")
        if not isinstance(client_id, str) or not client_id:
            raise AdapterFailure("pre-registration context did not contain client_id")
        return client_id
    return None


def _write_auth_registration(
    config_path: Path,
    *,
    server_url: str,
    oauth_client_id: str | None,
    oauth_client_secret_env_var: str | None = None,
) -> None:
    existing = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    block = f"\n[mcp_servers.{TEST_SERVER_NAME}]\nurl = {json.dumps(server_url, ensure_ascii=False)}\n"
    if oauth_client_id is not None:
        block += (
            f"\n[mcp_servers.{TEST_SERVER_NAME}.oauth]\n"
            f"client_id = {json.dumps(oauth_client_id, ensure_ascii=False)}\n"
        )
        if oauth_client_secret_env_var is not None:
            block += (
                "client_secret_env_var = "
                f"{json.dumps(oauth_client_secret_env_var, ensure_ascii=False)}\n"
            )
    config_path.write_text(existing.rstrip() + "\n" + block, encoding="utf-8")


def _validate_oauth_secret_not_persisted(codex_home: Path, client_secret: str) -> None:
    if not client_secret:
        return

    candidates = [codex_home / "config.toml", *codex_home.rglob(".credentials.json")]
    secret_bytes = client_secret.encode("utf-8")
    for candidate in dict.fromkeys(candidates):
        if not candidate.is_file():
            continue
        if secret_bytes in candidate.read_bytes():
            raise AdapterFailure(
                "environment-provided OAuth client secret was persisted in "
                f"{candidate.relative_to(codex_home)}"
            )


def _oauth_login(
    client: AppServerClient,
    *,
    scopes: Sequence[str] | None,
    timeout_seconds: float,
) -> tuple[bool, str | None]:
    event_index = len(client.events)
    params: dict[str, object] = {
        "name": TEST_SERVER_NAME,
        "timeoutSecs": max(1, round(timeout_seconds)),
    }
    if scopes is not None:
        params["scopes"] = list(scopes)
    response = client.request("mcpServer/oauth/login", params)
    result, detail = _response_result(response)
    if result is None:
        return False, detail
    authorization_url = result.get("authorizationUrl")
    if not isinstance(authorization_url, str):
        raise AdapterFailure("mcpServer/oauth/login returned no authorization URL")

    _drive_headless_authorization(
        authorization_url,
        timeout_seconds=timeout_seconds,
    )
    event = client.wait_for_notification(
        AUTH_COMPLETION_METHOD,
        predicate=lambda params: params.get("name") == TEST_SERVER_NAME,
        after_event_index=event_index,
    )
    params_value = event.get("params")
    if not isinstance(params_value, dict):
        raise AdapterFailure("OAuth completion notification did not contain params")
    success = params_value.get("success") is True
    error = params_value.get("error")
    return success, str(error) if error is not None else None


def _reload_mcp(client: AppServerClient) -> None:
    _result_or_raise(
        client.request("config/mcpServer/reload", None),
        "config/mcpServer/reload",
    )


def _auth_inventory(client: AppServerClient) -> dict[str, object]:
    inventory = _result_or_raise(
        client.request("mcpServerStatus/list", {"detail": "full"}),
        "mcpServerStatus/list",
    )
    _server_entry(inventory)
    return inventory


def _auth_tool_call(client: AppServerClient, workspace: Path) -> None:
    thread_id = _thread_id(client, workspace)
    _call(
        client,
        thread_id=thread_id,
        tool="test-tool",
        arguments={},
    )


def _login_reload_and_call(
    client: AppServerClient,
    *,
    workspace: Path,
    timeout_seconds: float,
    scopes: Sequence[str] | None = None,
) -> None:
    success, error = _oauth_login(
        client,
        scopes=scopes,
        timeout_seconds=timeout_seconds,
    )
    if not success:
        raise AdapterFailure(f"OAuth login failed: {error or 'unknown error'}")
    _reload_mcp(client)
    _auth_inventory(client)
    _auth_tool_call(client, workspace)


def _exercise_auth_scenario(
    client: AppServerClient,
    *,
    scenario: str,
    workspace: Path,
    timeout_seconds: float,
    require_automatic_auth: bool = False,
) -> str:
    if scenario in EXPECTED_AUTH_REJECTION_SCENARIOS:
        success, error = _oauth_login(
            client,
            scopes=None,
            timeout_seconds=timeout_seconds,
        )
        if success:
            raise AdapterFailure(
                "OAuth flow unexpectedly accepted authorization metadata that must be rejected"
            )
        return f"rejected invalid authorization flow: {error or 'request rejected'}"

    if scenario == "auth/scope-step-up":
        success, error = _oauth_login(
            client,
            scopes=None,
            timeout_seconds=timeout_seconds,
        )
        if not success:
            raise AdapterFailure(
                f"initial OAuth login failed: {error or 'unknown error'}"
            )
        _reload_mcp(client)
        _auth_inventory(client)
        if require_automatic_auth:
            _auth_tool_call(client, workspace)
            return "Codex automatically recovered from the challenged OAuth scope"
        try:
            _auth_tool_call(client, workspace)
        except AdapterFailure:
            pass
        else:
            raise AdapterFailure(
                "scope-step-up tool call did not request additional scope"
            )

        # The resource server challenges with only the missing scope. The Rust
        # client must union it with the previously granted scope.
        _login_reload_and_call(
            client,
            workspace=workspace,
            timeout_seconds=timeout_seconds,
            scopes=("mcp:write",),
        )
        return "completed initial and scope-upgrade authorization flows"

    if scenario == "auth/scope-retry-limit":
        if require_automatic_auth:
            success, error = _oauth_login(
                client,
                scopes=None,
                timeout_seconds=timeout_seconds,
            )
            if not success:
                raise AdapterFailure(
                    f"initial OAuth login failed: {error or 'unknown error'}"
                )
            _reload_mcp(client)
            _auth_inventory(client)
            try:
                _auth_tool_call(client, workspace)
            except AdapterFailure:
                return "observed Codex's production OAuth retry-limit behavior"
            raise AdapterFailure(
                "retry-limit scenario unexpectedly completed the tool call"
            )
        for attempt in range(3):
            success, error = _oauth_login(
                client,
                scopes=None if attempt == 0 else ("mcp:write",),
                timeout_seconds=timeout_seconds,
            )
            if not success:
                raise AdapterFailure(f"OAuth retry failed: {error or 'unknown error'}")
            _reload_mcp(client)
            _auth_inventory(client)
            try:
                _auth_tool_call(client, workspace)
            except AdapterFailure:
                continue
            raise AdapterFailure(
                "retry-limit scenario unexpectedly completed the tool call"
            )
        return "stopped after three unsuccessful authorization attempts"

    if scenario == "auth/authorization-server-migration":
        success, error = _oauth_login(
            client,
            scopes=None,
            timeout_seconds=timeout_seconds,
        )
        if not success:
            raise AdapterFailure(
                f"initial OAuth login failed: {error or 'unknown error'}"
            )
        _reload_mcp(client)
        _auth_inventory(client)
        if require_automatic_auth:
            _auth_tool_call(client, workspace)
            return (
                "Codex automatically registered with the migrated authorization server"
            )
        try:
            _auth_tool_call(client, workspace)
        except AdapterFailure:
            pass
        else:
            raise AdapterFailure("migration scenario did not require re-authorization")
        _login_reload_and_call(
            client,
            workspace=workspace,
            timeout_seconds=timeout_seconds,
        )
        return (
            "re-authorized after the protected resource changed authorization servers"
        )

    _login_reload_and_call(
        client,
        workspace=workspace,
        timeout_seconds=timeout_seconds,
    )
    return "completed OAuth login, authenticated discovery, and tool call"


def _exercise_scenario(
    client: AppServerClient,
    *,
    scenario: str,
    workspace: Path,
    inventory: Mapping[str, object],
) -> str:
    thread_id = _thread_id(client, workspace)

    if scenario == "tools_call":
        _call(
            client,
            thread_id=thread_id,
            tool="add_numbers",
            arguments={"a": 2, "b": 3},
        )
        return "called add_numbers"

    if scenario == "elicitation-sep1034-client-defaults":
        _call(
            client,
            thread_id=thread_id,
            tool="test_client_elicitation_defaults",
            arguments={},
        )
        return "completed legacy elicitation with omitted optional fields"

    if scenario == "sse-retry":
        _call(
            client,
            thread_id=thread_id,
            tool="test_reconnection",
            arguments={},
        )
        return "completed the SSE reconnection tool call"

    if scenario == "sep-2322-client-request-state":
        for tool in (
            "test_mrtr_unrelated",
            "test_mrtr_no_result_type",
            "test_mrtr_echo_state",
            "test_mrtr_no_state",
        ):
            _call(client, thread_id=thread_id, tool=tool, arguments={})
        return "completed all four MRTR flows"

    if scenario == "http-standard-headers":
        _call(
            client,
            thread_id=thread_id,
            tool="test_headers",
            arguments={},
        )
        entry = _server_entry(inventory)
        resources = entry.get("resources")
        if not isinstance(resources, list) or not resources:
            raise AdapterFailure("standard-header scenario exposed no resources")
        first = resources[0]
        if not isinstance(first, dict) or not isinstance(first.get("uri"), str):
            raise AdapterFailure("standard-header scenario resource had no URI")
        _result_or_raise(
            client.request(
                "mcpServer/resource/read",
                {
                    "threadId": thread_id,
                    "server": TEST_SERVER_NAME,
                    "uri": first["uri"],
                },
            ),
            "mcpServer/resource/read",
        )
        return "called a tool and read a resource"

    if scenario == "http-custom-headers":
        for tool, arguments in _context_tool_calls():
            _call(
                client,
                thread_id=thread_id,
                tool=tool,
                arguments=arguments,
            )
        return "called both custom-header tools with official values"

    if scenario == "http-invalid-tool-headers":
        _call(
            client,
            thread_id=thread_id,
            tool="valid_tool",
            arguments={"region": "us-west1"},
        )
        return "called the valid tool after filtering malformed definitions"

    if scenario in {
        "initialize",
        "request-metadata",
        "json-schema-ref-no-deref",
    }:
        # Discovery performed by mcpServerStatus/list is the behavior these
        # scenarios observe. Starting a thread also exercises the initialized
        # server through the same public app-server interface as the other
        # scenarios.
        return "completed MCP discovery"

    raise AdapterFailure(f"unsupported official scenario: {scenario}")


def run_adapter(server_url: str) -> dict[str, object]:
    codex_binary = _required_path("CODEX_CONFORMANCE_BINARY")
    codex_home = _required_path("CODEX_CONFORMANCE_HOME")
    scenario = _required_env("MCP_CONFORMANCE_SCENARIO")
    protocol_version = _required_env("MCP_CONFORMANCE_PROTOCOL_VERSION")
    timeout_seconds = float(os.environ.get("CODEX_CONFORMANCE_TIMEOUT", "30"))
    enable_modern_feature = (
        os.environ.get("CODEX_CONFORMANCE_ENABLE_MODERN_FEATURE", "1") != "0"
    )
    require_automatic_auth = (
        os.environ.get("CODEX_CONFORMANCE_REQUIRE_AUTOMATIC_AUTH", "0") == "1"
    )
    context = _conformance_context()

    codex_home.mkdir(parents=True, exist_ok=True)
    workspace = codex_home / "workspace"
    workspace.mkdir()
    env = _isolated_environment(codex_home)
    # Scenario context can contain ephemeral OAuth client secrets. The adapter
    # consumes it directly and does not expose the full blob to Codex.
    env.pop("MCP_CONFORMANCE_CONTEXT", None)
    steps: list[Step] = []
    registered = False
    error: str | None = None

    try:
        if scenario.startswith("auth/"):
            config_path = codex_home / "config.toml"
            config_path.write_text(
                'mcp_oauth_credentials_store = "file"\n',
                encoding="utf-8",
            )
            steps.append(
                Step(
                    "oauth_store_configuration",
                    True,
                    "configured the isolated file OAuth credential store",
                )
            )

        if protocol_version == MODERN_VERSION and enable_modern_feature:
            feature = _run_command(
                [
                    str(codex_binary),
                    "features",
                    "enable",
                    "mcp_2026_07_28",
                ],
                env=env,
                cwd=workspace,
                timeout_seconds=timeout_seconds,
            )
            feature_configured = feature.returncode == 0
            steps.append(
                Step(
                    "modern_feature_configuration",
                    feature_configured,
                    "configured mcp_2026_07_28 before MCP startup"
                    if feature_configured
                    else _command_detail(feature),
                )
            )
            if not feature_configured:
                raise AdapterFailure("could not configure the modern MCP feature")

        oauth_client_id = _oauth_client_id(
            scenario,
            context,
            require_production_client_identity=require_automatic_auth,
        )
        oauth_client_secret = context.get("client_secret")
        oauth_client_secret_env_var = None
        if (
            scenario == "auth/pre-registration"
            and isinstance(oauth_client_secret, str)
            and oauth_client_secret
        ):
            env[PRE_REGISTERED_CLIENT_SECRET_ENV_VAR] = oauth_client_secret
            oauth_client_secret_env_var = PRE_REGISTERED_CLIENT_SECRET_ENV_VAR
        if scenario.startswith("auth/"):
            _write_auth_registration(
                codex_home / "config.toml",
                server_url=server_url,
                oauth_client_id=oauth_client_id,
                oauth_client_secret_env_var=oauth_client_secret_env_var,
            )
            registered = True
            steps.append(
                Step(
                    "mcp_registration",
                    True,
                    "wrote isolated registration without triggering CLI auto-login",
                )
            )
        else:
            add = _run_command(
                [
                    str(codex_binary),
                    "mcp",
                    "add",
                    TEST_SERVER_NAME,
                    "--url",
                    server_url,
                ],
                env=env,
                cwd=workspace,
                timeout_seconds=timeout_seconds,
            )
            registered = add.returncode == 0
            steps.append(Step("mcp_add", registered, _command_detail(add)))
            if not registered:
                raise AdapterFailure("codex mcp add failed")

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
        registration_ok = False
        if get.returncode == 0:
            try:
                decoded = json.loads(get.stdout)
                transport = (
                    decoded.get("transport") if isinstance(decoded, dict) else None
                )
                registration_ok = (
                    isinstance(transport, dict)
                    and transport.get("type") == "streamable_http"
                    and transport.get("url") == server_url
                )
            except json.JSONDecodeError:
                pass
        steps.append(
            Step(
                "mcp_get",
                registration_ok,
                "registered official scenario URL"
                if registration_ok
                else _command_detail(get),
            )
        )
        if not registration_ok:
            raise AdapterFailure("Codex registration did not preserve the scenario URL")

        with AppServerClient(
            codex_binary,
            env=env,
            cwd=workspace,
            timeout_seconds=timeout_seconds,
            elicitation_content=lambda params: _elicitation_content(scenario, params),
        ) as client:
            initialize = _result_or_raise(
                client.request(
                    "initialize",
                    {
                        "clientInfo": {
                            "name": "official-mcp-conformance-adapter",
                            "title": "Official MCP conformance adapter",
                            "version": "1.0.0",
                        },
                        "capabilities": {
                            "experimentalApi": True,
                            "requestAttestation": False,
                            "mcpServerOpenaiFormElicitation": True,
                        },
                    },
                ),
                "app-server initialize",
            )
            steps.append(Step("app_server_initialize", bool(initialize), "initialized"))
            client.notify("initialized")

            if protocol_version == MODERN_VERSION and enable_modern_feature:
                feature = _result_or_raise(
                    client.request(
                        "experimentalFeature/enablement/set",
                        {"enablement": {"mcp_2026_07_28": True}},
                    ),
                    "experimentalFeature/enablement/set",
                )
                enabled = (
                    isinstance(feature.get("enablement"), dict)
                    and feature["enablement"].get("mcp_2026_07_28") is True
                )
                steps.append(
                    Step(
                        "modern_feature_enablement",
                        enabled,
                        "enabled mcp_2026_07_28"
                        if enabled
                        else f"unexpected response: {feature!r}",
                    )
                )
                if not enabled:
                    raise AdapterFailure("could not enable the modern MCP feature")

            if scenario.startswith("auth/"):
                detail = _exercise_auth_scenario(
                    client,
                    scenario=scenario,
                    workspace=workspace,
                    timeout_seconds=timeout_seconds,
                    require_automatic_auth=require_automatic_auth,
                )
                if scenario == "auth/pre-registration" and isinstance(
                    oauth_client_secret,
                    str,
                ):
                    _validate_oauth_secret_not_persisted(
                        codex_home, oauth_client_secret
                    )
                    steps.append(
                        Step(
                            "oauth_client_secret_not_persisted",
                            True,
                            "environment-provided confidential-client secret was not "
                            "written to configuration or file-backed credentials",
                        )
                    )
                steps.append(Step("authentication", True, detail))
            else:
                inventory = _result_or_raise(
                    client.request("mcpServerStatus/list", {"detail": "full"}),
                    "mcpServerStatus/list",
                )
                _server_entry(inventory)
                steps.append(Step("inventory", True, "official server discovered"))
                detail = _exercise_scenario(
                    client,
                    scenario=scenario,
                    workspace=workspace,
                    inventory=inventory,
                )
            steps.append(Step("scenario", True, detail))
    except (AdapterFailure, AppServerError, OSError, ValueError) as exc:
        error = str(exc)
        if not steps or steps[-1].success:
            steps.append(Step("scenario", False, error))
    finally:
        if registered:
            remove = _run_command(
                [str(codex_binary), "mcp", "remove", TEST_SERVER_NAME],
                env=env,
                cwd=workspace,
                timeout_seconds=timeout_seconds,
            )
            steps.append(
                Step("mcp_remove", remove.returncode == 0, _command_detail(remove))
            )

    success = bool(steps) and all(step.success for step in steps)
    return {
        "success": success,
        "scenario": scenario,
        "protocolVersion": protocol_version,
        "automaticAuthRequired": require_automatic_auth,
        "serverUrl": server_url,
        "steps": [asdict(step) for step in steps],
        "error": error,
    }


def main(argv: Sequence[str] | None = None) -> int:
    values = list(sys.argv[1:] if argv is None else argv)
    report_path_value = os.environ.get("CODEX_CONFORMANCE_ADAPTER_REPORT")
    report: dict[str, object]
    try:
        if len(values) != 1:
            raise AdapterFailure("expected exactly one official scenario server URL")
        report = run_adapter(values[0])
    except Exception as exc:  # Preserve a diagnostic artifact for the parent.
        report = {
            "success": False,
            "error": str(exc),
            "traceback": traceback.format_exc(limit=20),
            "steps": [],
        }

    if report_path_value:
        report_path = Path(report_path_value)
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(
            json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 0 if report.get("success") is True else 1


if __name__ == "__main__":
    raise SystemExit(main())
