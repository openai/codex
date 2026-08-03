import base64
import http.client
import io
import json
import threading
from collections.abc import Iterator
from contextlib import contextmanager
from unittest.mock import patch

import pytest

from server import (
    CATALOG_MAX_PROFILE,
    CATALOG_OVER_LIMIT_PROFILE,
    DEFAULT_PROFILE,
    HEADER_MISMATCH,
    INVALID_PARAMS,
    LEGACY_VERSION,
    MAX_CATALOG_ITEMS,
    MISMATCHED_DISCOVERY_ID_PROFILE,
    MISSING_REQUIRED_CLIENT_CAPABILITY,
    MODERN_VERSION,
    NULL_DISCOVERY_ID_PROFILE,
    REPEATED_CURSOR_PROFILE,
    RESOURCE_URIS,
    REVIEW_EXACT_INTEGER,
    REVIEW_MAX_INTEGER,
    REVIEW_MRTR_INPUT_REQUEST_COUNT,
    REVIEW_PAGE_CURSOR,
    REVIEW_PROFILE,
    REVIEW_SSE_COMMENT_LINE_BYTES,
    REVIEW_SSE_COMMENT_LINE_COUNT,
    REVIEW_SSE_EVENT_LIMIT_BYTES,
    SHIPPING_LEGACY_VERSION,
    SSE_COMMENT_FLOOD_PROFILE,
    SSE_CR_COMMENTS_PROFILE,
    UNSUPPORTED_PROTOCOL_VERSION,
    ConnectionState,
    ProtocolServer,
    ResponsePlan,
    _encode_sse_messages,
    _parse_args,
    make_http_server,
    run_stdio,
)


def modern_meta(
    *,
    version: str = MODERN_VERSION,
    capabilities: dict[str, object] | None = None,
) -> dict[str, object]:
    return {
        "io.modelcontextprotocol/protocolVersion": version,
        "io.modelcontextprotocol/clientInfo": {
            "name": "fixture-test-client",
            "version": "1.0.0",
        },
        "io.modelcontextprotocol/clientCapabilities": capabilities or {},
    }


def request(
    method: str,
    *,
    request_id: int = 1,
    params: dict[str, object] | None = None,
) -> dict[str, object]:
    return {
        "jsonrpc": "2.0",
        "id": request_id,
        "method": method,
        "params": {"_meta": modern_meta()} if params is None else params,
    }


def result(plan: ResponsePlan) -> dict[str, object]:
    response = plan.response
    assert isinstance(response, dict)
    value = response.get("result")
    assert isinstance(value, dict)
    return value


def error(plan: ResponsePlan) -> dict[str, object]:
    response = plan.response
    assert isinstance(response, dict)
    value = response.get("error")
    assert isinstance(value, dict)
    return value


def initialize_legacy(
    server: ProtocolServer,
    state: ConnectionState,
    *,
    version: str = LEGACY_VERSION,
) -> None:
    server.handle(
        request(
            "initialize",
            params={
                "protocolVersion": version,
                "capabilities": {},
                "clientInfo": {"name": "legacy-client", "version": "1.0.0"},
            },
        ),
        state,
    )
    server.handle(
        {
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {},
        },
        state,
    )


def test_modern_discovery_is_stateless_and_cacheable() -> None:
    server = ProtocolServer(MODERN_VERSION)

    plan = server.handle(request("server/discover"), ConnectionState())
    value = result(plan)

    assert value["resultType"] == "complete"
    assert value["supportedVersions"] == [MODERN_VERSION]
    assert value["ttlMs"] == 60_000
    assert value["cacheScope"] == "public"
    assert value["_meta"] == {
        "io.modelcontextprotocol/serverInfo": {
            "name": "openai-mcp-spec-test-server",
            "version": "0.1.0",
        }
    }


def test_modern_request_requires_metadata() -> None:
    server = ProtocolServer(MODERN_VERSION)

    plan = server.handle(
        request("tools/list", params={}),
        ConnectionState(),
    )

    assert error(plan)["code"] == INVALID_PARAMS
    assert plan.http_status == 400


def test_modern_request_reports_supported_version() -> None:
    server = ProtocolServer(MODERN_VERSION)
    message = request(
        "tools/list",
        params={"_meta": modern_meta(version=LEGACY_VERSION)},
    )

    plan = server.handle(message, ConnectionState())

    assert error(plan) == {
        "code": UNSUPPORTED_PROTOCOL_VERSION,
        "message": "Unsupported protocol version",
        "data": {
            "supported": [MODERN_VERSION],
            "requested": LEGACY_VERSION,
        },
    }


def test_legacy_probe_falls_back_then_uses_initialize() -> None:
    server = ProtocolServer(LEGACY_VERSION)
    state = ConnectionState()

    probe = server.handle(request("server/discover"), state)
    assert error(probe)["code"] == -32601

    initialize_legacy(server, state)
    tools = result(server.handle(request("tools/list", params={}), state))

    assert "resultType" not in tools
    assert "ttlMs" not in tools
    assert [tool["name"] for tool in tools["tools"]] == sorted(
        tool["name"] for tool in tools["tools"]
    )


def test_shipping_legacy_mode_negotiates_the_real_product_protocol() -> None:
    server = ProtocolServer(SHIPPING_LEGACY_VERSION)
    state = ConnectionState()

    plan = server.handle(
        request(
            "initialize",
            params={
                "protocolVersion": SHIPPING_LEGACY_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "shipping-client", "version": "1.0.0"},
            },
        ),
        state,
    )

    assert result(plan)["protocolVersion"] == "2025-06-18"
    assert result(plan)["instructions"] == "Legacy 2025-06-18 compatibility fixture."
    assert "resultType" not in result(plan)
    assert state.initialize_seen


def test_shipping_legacy_discovery_falls_back_to_initialized_legacy_tools() -> None:
    server = ProtocolServer(SHIPPING_LEGACY_VERSION)
    state = ConnectionState()

    probe = server.handle(request("server/discover"), state)

    assert error(probe)["code"] == -32601

    initialize_legacy(server, state, version=SHIPPING_LEGACY_VERSION)
    tools = result(server.handle(request("tools/list", params={}), state))

    assert "resultType" not in tools
    assert "ttlMs" not in tools
    assert [tool["name"] for tool in tools["tools"]] == [
        "client_metadata",
        "echo",
        "fail",
        "header_echo",
        "progress",
    ]


def test_cli_accepts_the_shipping_legacy_protocol_mode() -> None:
    args = _parse_args(["--mode", SHIPPING_LEGACY_VERSION, "--transport", "stdio"])

    assert args.mode == "2025-06-18"
    assert args.transport == "stdio"
    assert args.profile == DEFAULT_PROFILE


@pytest.mark.parametrize(
    "mode", (SHIPPING_LEGACY_VERSION, LEGACY_VERSION, MODERN_VERSION)
)
@pytest.mark.parametrize("transport", ("stdio", "http"))
@pytest.mark.parametrize("profile", (CATALOG_MAX_PROFILE, CATALOG_OVER_LIMIT_PROFILE))
def test_catalog_boundary_profiles_are_available_from_the_cli(
    mode: str, transport: str, profile: str
) -> None:
    args = _parse_args(["--mode", mode, "--transport", transport, "--profile", profile])

    assert (args.mode, args.transport, args.profile) == (mode, transport, profile)


def test_mrtr_requires_capability_and_echoes_state() -> None:
    server = ProtocolServer(MODERN_VERSION)
    tools = result(server.handle(request("tools/list"), ConnectionState()))["tools"]
    tools_by_name = {tool["name"]: tool for tool in tools}
    assert "outputSchema" not in tools_by_name["request_input"]
    assert "outputSchema" in tools_by_name["echo"]

    first = request(
        "tools/call",
        params={
            "_meta": modern_meta(),
            "name": "request_input",
            "arguments": {},
        },
    )

    missing_capability = server.handle(first, ConnectionState())
    assert error(missing_capability)["code"] == MISSING_REQUIRED_CLIENT_CAPABILITY

    first["params"]["_meta"] = modern_meta(capabilities={"elicitation": {"form": {}}})
    interim = result(server.handle(first, ConnectionState()))
    assert interim["resultType"] == "input_required"
    assert interim["_meta"] == {
        "io.modelcontextprotocol/serverInfo": {
            "name": "openai-mcp-spec-test-server",
            "version": "0.1.0",
        }
    }
    assert "structuredContent" not in interim
    assert interim["requestState"] == "opaque:request_input:v1"
    assert interim["inputRequests"]["confirmation"]["method"] == "elicitation/create"

    retry = request(
        "tools/call",
        request_id=2,
        params={
            "_meta": modern_meta(capabilities={"elicitation": {"form": {}}}),
            "name": "request_input",
            "arguments": {},
            "requestState": interim["requestState"],
            "inputResponses": {
                "confirmation": {
                    "action": "accept",
                    "content": {"confirmation": "confirmed"},
                }
            },
        },
    )
    completed = result(server.handle(retry, ConnectionState()))

    assert completed["resultType"] == "complete"
    assert completed["structuredContent"] == {"confirmation": "confirmed"}


def test_subscription_is_acknowledged_tagged_and_closed() -> None:
    server = ProtocolServer(MODERN_VERSION)
    plan = server.handle(
        request(
            "subscriptions/listen",
            request_id=42,
            params={
                "_meta": modern_meta(),
                "notifications": {
                    "toolsListChanged": True,
                    "resourceSubscriptions": [RESOURCE_URIS[0], "test://unknown"],
                },
            },
        ),
        ConnectionState(),
    )

    assert plan.force_sse
    assert plan.notifications[0]["method"] == (
        "notifications/subscriptions/acknowledged"
    )
    for notification in plan.notifications:
        assert (
            notification["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"]
            == 42
        )
    assert result(plan)["_meta"]["io.modelcontextprotocol/subscriptionId"] == 42


def test_progress_and_logging_are_per_request_opt_ins() -> None:
    server = ProtocolServer(MODERN_VERSION)
    message = request(
        "tools/call",
        params={
            "_meta": {
                **modern_meta(),
                "progressToken": "progress-1",
                "io.modelcontextprotocol/logLevel": "info",
            },
            "name": "progress",
            "arguments": {},
        },
    )

    plan = server.handle(message, ConnectionState())

    assert [notification["method"] for notification in plan.notifications] == [
        "notifications/progress",
        "notifications/message",
    ]
    assert result(plan)["resultType"] == "complete"


def test_stdio_supports_both_eras() -> None:
    modern_input = io.StringIO(
        json.dumps(request("tools/list"), separators=(",", ":")) + "\n"
    )
    modern_output = io.StringIO()
    run_stdio(ProtocolServer(MODERN_VERSION), modern_input, modern_output)
    modern_response = json.loads(modern_output.getvalue())
    assert modern_response["result"]["resultType"] == "complete"

    legacy_messages = [
        request(
            "initialize",
            params={
                "protocolVersion": LEGACY_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "legacy", "version": "1.0.0"},
            },
        ),
        {
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {},
        },
        request("tools/list", request_id=2, params={}),
    ]
    legacy_input = io.StringIO(
        "".join(
            json.dumps(item, separators=(",", ":")) + "\n" for item in legacy_messages
        )
    )
    legacy_output = io.StringIO()
    run_stdio(ProtocolServer(LEGACY_VERSION), legacy_input, legacy_output)
    responses = [json.loads(line) for line in legacy_output.getvalue().splitlines()]
    assert len(responses) == 2
    assert "resultType" not in responses[1]["result"]


def test_stdio_negotiates_and_lists_tools_for_the_real_shipping_legacy_protocol() -> (
    None
):
    messages = [
        request(
            "initialize",
            params={
                "protocolVersion": SHIPPING_LEGACY_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "shipping-legacy", "version": "1.0.0"},
            },
        ),
        {
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {},
        },
        request("tools/list", request_id=2, params={}),
    ]
    stdin = io.StringIO(
        "".join(
            json.dumps(message, separators=(",", ":")) + "\n" for message in messages
        )
    )
    stdout = io.StringIO()

    run_stdio(ProtocolServer(SHIPPING_LEGACY_VERSION), stdin, stdout)

    responses = [json.loads(line) for line in stdout.getvalue().splitlines()]
    assert len(responses) == 2
    assert responses[0]["result"]["protocolVersion"] == "2025-06-18"
    assert [tool["name"] for tool in responses[1]["result"]["tools"]] == [
        "client_metadata",
        "echo",
        "fail",
        "header_echo",
        "progress",
    ]
    assert "resultType" not in responses[1]["result"]


def test_default_profile_preserves_existing_legacy_and_modern_tool_lists() -> None:
    modern = ProtocolServer(MODERN_VERSION)
    modern_tools = result(modern.handle(request("tools/list"), ConnectionState()))

    assert modern.profile == DEFAULT_PROFILE
    assert [tool["name"] for tool in modern_tools["tools"]] == [
        "client_metadata",
        "echo",
        "fail",
        "header_echo",
        "progress",
        "request_input",
    ]
    assert "nextCursor" not in modern_tools

    legacy = ProtocolServer(LEGACY_VERSION)
    legacy_state = ConnectionState()
    initialize_legacy(legacy, legacy_state)
    legacy_tools = result(legacy.handle(request("tools/list", params={}), legacy_state))

    assert [tool["name"] for tool in legacy_tools["tools"]] == [
        "client_metadata",
        "echo",
        "fail",
        "header_echo",
        "progress",
    ]
    assert "nextCursor" not in legacy_tools


@pytest.mark.parametrize(
    "mode", (SHIPPING_LEGACY_VERSION, LEGACY_VERSION, MODERN_VERSION)
)
@pytest.mark.parametrize(
    ("profile", "expected_size"),
    (
        (CATALOG_MAX_PROFILE, MAX_CATALOG_ITEMS),
        (CATALOG_OVER_LIMIT_PROFILE, MAX_CATALOG_ITEMS + 1),
    ),
)
def test_catalog_boundary_profiles_return_exact_valid_unique_tool_counts(
    mode: str, profile: str, expected_size: int
) -> None:
    server = ProtocolServer(mode, profile=profile)
    state = ConnectionState()
    if mode != MODERN_VERSION:
        initialize_legacy(server, state, version=mode)

    params = {"_meta": modern_meta()} if mode == MODERN_VERSION else {}
    catalog = result(server.handle(request("tools/list", params=params), state))
    tools = catalog["tools"]
    names = [tool["name"] for tool in tools]

    assert len(tools) == expected_size
    assert len(set(names)) == expected_size
    assert names == sorted(names)
    assert {"client_metadata", "echo", "fail", "header_echo", "progress"} <= set(names)
    assert all(tool["inputSchema"]["type"] == "object" for tool in tools)
    assert "nextCursor" not in catalog
    if mode == MODERN_VERSION:
        assert "request_input" in names
        assert catalog["resultType"] == "complete"
    else:
        assert "resultType" not in catalog


@pytest.mark.parametrize(
    "mode", (SHIPPING_LEGACY_VERSION, LEGACY_VERSION, MODERN_VERSION)
)
@pytest.mark.parametrize(
    ("profile", "expected_size"),
    (
        (CATALOG_MAX_PROFILE, MAX_CATALOG_ITEMS),
        (CATALOG_OVER_LIMIT_PROFILE, MAX_CATALOG_ITEMS + 1),
    ),
)
def test_catalog_boundary_profiles_round_trip_over_real_stdio(
    mode: str, profile: str, expected_size: int
) -> None:
    messages: list[dict[str, object]] = []
    if mode != MODERN_VERSION:
        messages.extend(
            [
                request(
                    "initialize",
                    params={
                        "protocolVersion": mode,
                        "capabilities": {},
                        "clientInfo": {"name": "catalog-boundary", "version": "1.0.0"},
                    },
                ),
                {
                    "jsonrpc": "2.0",
                    "method": "notifications/initialized",
                    "params": {},
                },
            ]
        )
    messages.append(
        request(
            "tools/list",
            request_id=2,
            params={"_meta": modern_meta()} if mode == MODERN_VERSION else {},
        )
    )
    stdin = io.StringIO(
        "".join(
            json.dumps(message, separators=(",", ":")) + "\n" for message in messages
        )
    )
    stdout = io.StringIO()

    run_stdio(ProtocolServer(mode, profile=profile), stdin, stdout)

    catalog = json.loads(stdout.getvalue().splitlines()[-1])["result"]
    names = [tool["name"] for tool in catalog["tools"]]
    assert len(names) == expected_size
    assert len(set(names)) == expected_size
    assert "nextCursor" not in catalog
    assert ("resultType" in catalog) is (mode == MODERN_VERSION)


def test_review_profile_hides_regression_tools_on_the_second_page() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE)

    first = result(server.handle(request("tools/list"), ConnectionState()))
    assert [tool["name"] for tool in first["tools"]] == ["client_metadata", "echo"]
    assert first["nextCursor"] == REVIEW_PAGE_CURSOR
    assert first["resultType"] == "complete"

    second = result(
        server.handle(
            request(
                "tools/list",
                request_id=2,
                params={"_meta": modern_meta(), "cursor": first["nextCursor"]},
            ),
            ConnectionState(),
        )
    )

    assert [tool["name"] for tool in second["tools"]] == [
        "fail",
        "header_echo",
        "progress",
        "request_input",
        "review_integer_elicitation",
        "review_large_integer",
        "review_mrtr_cap",
        "review_protocol_env",
    ]
    assert "nextCursor" not in second


def test_review_profile_rejects_an_invalid_tool_cursor() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE)

    plan = server.handle(
        request(
            "tools/list",
            params={"_meta": modern_meta(), "cursor": "not-a-review-cursor"},
        ),
        ConnectionState(),
    )

    assert error(plan) == {
        "code": INVALID_PARAMS,
        "message": "Invalid tool cursor",
    }


def test_repeated_cursor_profile_deterministically_repeats_the_cursor() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REPEATED_CURSOR_PROFILE)

    first = result(server.handle(request("tools/list"), ConnectionState()))
    second = result(
        server.handle(
            request(
                "tools/list",
                request_id=2,
                params={"_meta": modern_meta(), "cursor": first["nextCursor"]},
            ),
            ConnectionState(),
        )
    )

    assert first["nextCursor"] == REVIEW_PAGE_CURSOR
    assert second["nextCursor"] == REVIEW_PAGE_CURSOR
    assert any(tool["name"] == "review_large_integer" for tool in second["tools"])


def test_review_large_integer_schema_preserves_exact_json_integer_boundaries() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE)
    page = result(
        server.handle(
            request(
                "tools/list",
                params={"_meta": modern_meta(), "cursor": REVIEW_PAGE_CURSOR},
            ),
            ConnectionState(),
        )
    )
    definition = next(
        tool for tool in page["tools"] if tool["name"] == "review_large_integer"
    )
    schema = definition["inputSchema"]["properties"]["value"]

    assert schema == {
        "type": "integer",
        "minimum": 9_007_199_254_740_993,
        "maximum": 9_223_372_036_854_775_807,
        "default": 9_007_199_254_740_993,
    }
    assert json.loads(json.dumps(schema)) == schema
    assert REVIEW_EXACT_INTEGER > 2**53
    assert REVIEW_MAX_INTEGER == 2**63 - 1


def test_review_large_integer_tool_echoes_without_float_rounding() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE)

    plan = server.handle(
        request(
            "tools/call",
            params={
                "_meta": modern_meta(),
                "name": "review_large_integer",
                "arguments": {"value": REVIEW_EXACT_INTEGER},
            },
        ),
        ConnectionState(),
    )

    assert result(plan)["structuredContent"] == {"value": 9_007_199_254_740_993}
    assert json.loads(result(plan)["content"][0]["text"]) == {
        "value": 9_007_199_254_740_993
    }


def test_review_large_integer_rejects_floats_booleans_and_out_of_range_values() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE)

    for invalid_value in (
        float(REVIEW_EXACT_INTEGER),
        True,
        REVIEW_EXACT_INTEGER - 1,
        REVIEW_MAX_INTEGER + 1,
        str(REVIEW_EXACT_INTEGER),
    ):
        plan = server.handle(
            request(
                "tools/call",
                params={
                    "_meta": modern_meta(),
                    "name": "review_large_integer",
                    "arguments": {"value": invalid_value},
                },
            ),
            ConnectionState(),
        )

        assert error(plan)["code"] == INVALID_PARAMS


def test_review_large_integer_survives_stdio_json_serialization() -> None:
    message = request(
        "tools/call",
        params={
            "_meta": modern_meta(),
            "name": "review_large_integer",
            "arguments": {"value": REVIEW_EXACT_INTEGER},
        },
    )
    stdin = io.StringIO(json.dumps(message, separators=(",", ":")) + "\n")
    stdout = io.StringIO()

    run_stdio(ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE), stdin, stdout)

    raw_response = stdout.getvalue()
    assert "9007199254740993" in raw_response
    assert json.loads(raw_response)["result"]["structuredContent"] == {
        "value": 9_007_199_254_740_993
    }


def test_review_integer_elicitation_preserves_the_exact_requested_form_schema() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE)

    plan = server.handle(
        request(
            "tools/call",
            params={
                "_meta": modern_meta(capabilities={"elicitation": {"form": {}}}),
                "name": "review_integer_elicitation",
                "arguments": {},
            },
        ),
        ConnectionState(),
    )
    value = result(plan)
    pending = value["inputRequests"]["large_integer"]

    assert value["resultType"] == "input_required"
    assert value["requestState"] == "opaque:review_integer_elicitation:v1"
    assert pending["method"] == "elicitation/create"
    assert pending["params"]["requestedSchema"]["properties"]["value"] == {
        "type": "integer",
        "minimum": 9_007_199_254_740_993,
        "maximum": 9_223_372_036_854_775_807,
        "default": 9_007_199_254_740_993,
    }
    assert "9007199254740993" in json.dumps(value)


def test_review_integer_elicitation_round_trip_keeps_the_exact_integer() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE)
    meta = modern_meta(capabilities={"elicitation": {"form": {}}})

    first = result(
        server.handle(
            request(
                "tools/call",
                params={
                    "_meta": meta,
                    "name": "review_integer_elicitation",
                    "arguments": {},
                },
            ),
            ConnectionState(),
        )
    )
    completed = result(
        server.handle(
            request(
                "tools/call",
                request_id=2,
                params={
                    "_meta": meta,
                    "name": "review_integer_elicitation",
                    "arguments": {},
                    "requestState": first["requestState"],
                    "inputResponses": {
                        "large_integer": {
                            "action": "accept",
                            "content": {"value": REVIEW_EXACT_INTEGER},
                        }
                    },
                },
            ),
            ConnectionState(),
        )
    )

    assert completed["resultType"] == "complete"
    assert completed["structuredContent"] == {"value": 9_007_199_254_740_993}
    assert completed["content"] == [{"type": "text", "text": "9007199254740993"}]


def test_review_integer_elicitation_rejects_float_rounded_form_response() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE)

    plan = server.handle(
        request(
            "tools/call",
            params={
                "_meta": modern_meta(capabilities={"elicitation": {"form": {}}}),
                "name": "review_integer_elicitation",
                "arguments": {},
                "requestState": "opaque:review_integer_elicitation:v1",
                "inputResponses": {
                    "large_integer": {
                        "action": "accept",
                        "content": {"value": float(REVIEW_EXACT_INTEGER)},
                    }
                },
            },
        ),
        ConnectionState(),
    )

    assert error(plan)["code"] == INVALID_PARAMS


def test_review_integer_elicitation_requires_advertised_form_capability() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE)

    plan = server.handle(
        request(
            "tools/call",
            params={
                "_meta": modern_meta(),
                "name": "review_integer_elicitation",
                "arguments": {},
            },
        ),
        ConnectionState(),
    )

    assert error(plan)["code"] == MISSING_REQUIRED_CLIENT_CAPABILITY
    assert plan.http_status == 400


def test_review_mrtr_cap_returns_65_deterministic_pending_requests() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE)

    plan = server.handle(
        request(
            "tools/call",
            params={
                "_meta": modern_meta(capabilities={"elicitation": {"form": {}}}),
                "name": "review_mrtr_cap",
                "arguments": {},
            },
        ),
        ConnectionState(),
    )
    value = result(plan)

    assert value["resultType"] == "input_required"
    assert value["requestState"] == "opaque:review_mrtr_cap:v1"
    assert len(value["inputRequests"]) == REVIEW_MRTR_INPUT_REQUEST_COUNT == 65
    assert list(value["inputRequests"]) == [
        f"review-input-{index:02d}" for index in range(65)
    ]
    assert all(
        pending["method"] == "elicitation/create"
        for pending in value["inputRequests"].values()
    )


def test_review_mrtr_cap_requires_advertised_elicitation_capability() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE)

    plan = server.handle(
        request(
            "tools/call",
            params={
                "_meta": modern_meta(),
                "name": "review_mrtr_cap",
                "arguments": {},
            },
        ),
        ConnectionState(),
    )

    assert error(plan)["code"] == MISSING_REQUIRED_CLIENT_CAPABILITY
    assert plan.http_status == 400


def test_review_protocol_env_is_available_to_a_legacy_client() -> None:
    server = ProtocolServer(LEGACY_VERSION, profile=REVIEW_PROFILE)
    state = ConnectionState()
    initialize_legacy(server, state)
    tools = result(server.handle(request("tools/list", params={}), state))

    assert any(tool["name"] == "review_protocol_env" for tool in tools["tools"])

    with patch.dict(
        "os.environ", {"CODEX_MCP_PROTOCOL_VERSION": "legacy-review-sentinel"}
    ):
        plan = server.handle(
            request(
                "tools/call",
                params={"name": "review_protocol_env", "arguments": {}},
            ),
            state,
        )

    assert result(plan)["structuredContent"] == {"value": "legacy-review-sentinel"}


def test_review_protocol_env_is_available_to_the_shipping_legacy_client() -> None:
    server = ProtocolServer(SHIPPING_LEGACY_VERSION, profile=REVIEW_PROFILE)
    state = ConnectionState()
    initialize_legacy(server, state, version=SHIPPING_LEGACY_VERSION)
    tools = result(server.handle(request("tools/list", params={}), state))

    assert any(tool["name"] == "review_protocol_env" for tool in tools["tools"])

    with patch.dict(
        "os.environ", {"CODEX_MCP_PROTOCOL_VERSION": "shipping-legacy-sentinel"}
    ):
        plan = server.handle(
            request(
                "tools/call",
                params={"name": "review_protocol_env", "arguments": {}},
            ),
            state,
        )

    assert result(plan)["structuredContent"] == {"value": "shipping-legacy-sentinel"}


def test_review_protocol_env_is_available_to_a_modern_client() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=REVIEW_PROFILE)

    with patch.dict("os.environ", {"CODEX_MCP_PROTOCOL_VERSION": MODERN_VERSION}):
        plan = server.handle(
            request(
                "tools/call",
                params={
                    "_meta": modern_meta(),
                    "name": "review_protocol_env",
                    "arguments": {},
                },
            ),
            ConnectionState(),
        )

    assert result(plan)["structuredContent"] == {"value": MODERN_VERSION}


def test_mismatched_discovery_profile_returns_the_wrong_jsonrpc_id() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=MISMATCHED_DISCOVERY_ID_PROFILE)

    plan = server.handle(request("server/discover", request_id=42), ConnectionState())

    assert plan.response is not None
    assert plan.response["id"] == "review-mismatched-42"
    assert result(plan)["supportedVersions"] == [MODERN_VERSION]


def test_null_discovery_profile_returns_a_null_jsonrpc_id() -> None:
    server = ProtocolServer(MODERN_VERSION, profile=NULL_DISCOVERY_ID_PROFILE)

    plan = server.handle(request("server/discover", request_id=42), ConnectionState())

    assert plan.response is not None
    assert plan.response["id"] is None
    assert result(plan)["supportedVersions"] == [MODERN_VERSION]


def test_sse_comment_flood_profile_is_available_from_the_cli() -> None:
    args = _parse_args(
        [
            "--mode",
            MODERN_VERSION,
            "--transport",
            "http",
            "--profile",
            SSE_COMMENT_FLOOD_PROFILE,
        ]
    )

    assert args.profile == "sse-comment-flood"


def test_sse_comment_flood_is_valid_bounded_and_exceeds_the_data_event_limit() -> None:
    message: dict[str, object] = {"jsonrpc": "2.0", "id": 42, "result": {}}

    body = _encode_sse_messages([message], profile=SSE_COMMENT_FLOOD_PROFILE)

    assert REVIEW_SSE_COMMENT_LINE_COUNT == 4_097
    assert len(body) > REVIEW_SSE_EVENT_LIMIT_BYTES
    assert len(body) < (
        REVIEW_SSE_EVENT_LIMIT_BYTES + REVIEW_SSE_COMMENT_LINE_BYTES + 4_096
    )
    assert body.count(b": reviewer keepalive ") == REVIEW_SSE_COMMENT_LINE_COUNT
    assert b"\n" not in body
    assert body.endswith(b"\r\r")
    assert json.loads(body.rsplit(b"data: ", 1)[1].removesuffix(b"\r\r")) == message


@contextmanager
def running_http_server(
    mode: str,
    *,
    profile: str = DEFAULT_PROFILE,
) -> Iterator[tuple[str, int]]:
    server = make_http_server(ProtocolServer(mode, profile=profile), "127.0.0.1", 0)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        host, port = server.server_address
        yield str(host), int(port)
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def post_json(
    address: tuple[str, int],
    message: dict[str, object],
    headers: dict[str, str],
) -> tuple[int, dict[str, str], dict[str, object] | None]:
    connection = http.client.HTTPConnection(*address, timeout=2)
    body = json.dumps(message, ensure_ascii=False, separators=(",", ":")).encode()
    connection.request(
        "POST",
        "/mcp",
        body=body,
        headers={"Content-Type": "application/json", **headers},
    )
    response = connection.getresponse()
    response_body = response.read()
    response_headers = {name.lower(): value for name, value in response.getheaders()}
    connection.close()
    return (
        response.status,
        response_headers,
        json.loads(response_body) if response_body else None,
    )


def modern_headers(
    method: str,
    *,
    name: str | None = None,
    extra: dict[str, str] | None = None,
) -> dict[str, str]:
    headers = {
        "Accept": "application/json, text/event-stream",
        "MCP-Protocol-Version": MODERN_VERSION,
        "Mcp-Method": method,
    }
    if name is not None:
        headers["Mcp-Name"] = name
    if extra is not None:
        headers.update(extra)
    return headers


@pytest.mark.parametrize(
    "mode", (SHIPPING_LEGACY_VERSION, LEGACY_VERSION, MODERN_VERSION)
)
@pytest.mark.parametrize(
    ("profile", "expected_size"),
    (
        (CATALOG_MAX_PROFILE, MAX_CATALOG_ITEMS),
        (CATALOG_OVER_LIMIT_PROFILE, MAX_CATALOG_ITEMS + 1),
    ),
)
def test_catalog_boundary_profiles_round_trip_over_localhost_http(
    mode: str, profile: str, expected_size: int
) -> None:
    with running_http_server(mode, profile=profile) as address:
        if mode == MODERN_VERSION:
            status, _, body = post_json(
                address,
                request("tools/list"),
                modern_headers("tools/list"),
            )
        else:
            status, headers, body = post_json(
                address,
                request(
                    "initialize",
                    params={
                        "protocolVersion": mode,
                        "capabilities": {},
                        "clientInfo": {"name": "catalog-boundary", "version": "1.0.0"},
                    },
                ),
                {},
            )
            assert status == 200
            assert body is not None
            assert body["result"]["protocolVersion"] == mode
            session_headers = {"Mcp-Session-Id": headers["mcp-session-id"]}

            initialized_status, _, initialized_body = post_json(
                address,
                {
                    "jsonrpc": "2.0",
                    "method": "notifications/initialized",
                    "params": {},
                },
                session_headers,
            )
            assert initialized_status == 202
            assert initialized_body is None

            status, _, body = post_json(
                address,
                request("tools/list", request_id=2, params={}),
                session_headers,
            )

        assert status == 200
        assert body is not None
        catalog = body["result"]
        names = [tool["name"] for tool in catalog["tools"]]
        assert len(names) == expected_size
        assert len(set(names)) == expected_size
        assert "nextCursor" not in catalog
        assert ("resultType" in catalog) is (mode == MODERN_VERSION)


def test_modern_http_validates_standard_and_custom_headers() -> None:
    with running_http_server(MODERN_VERSION) as address:
        message = request(
            "tools/call",
            params={
                "_meta": modern_meta(),
                "name": "header_echo",
                "arguments": {
                    "region": "us-west1",
                    "attempt": 3,
                    "enabled": True,
                    "greeting": "Hello, 世界",
                },
            },
        )
        encoded_greeting = base64.b64encode("Hello, 世界".encode()).decode()
        headers = modern_headers(
            "tools/call",
            name="header_echo",
            extra={
                "Mcp-Param-Region": "us-west1",
                "Mcp-Param-Attempt": "3",
                "Mcp-Param-Enabled": "true",
                "Mcp-Param-Greeting": f"=?base64?{encoded_greeting}?=",
            },
        )

        status, _, body = post_json(address, message, headers)
        assert status == 200
        assert body["result"]["resultType"] == "complete"

        headers["Mcp-Name"] = "different"
        status, _, body = post_json(address, message, headers)
        assert status == 400
        assert body["error"]["code"] == HEADER_MISMATCH


def test_modern_http_accepts_base64_mcp_name() -> None:
    with running_http_server(MODERN_VERSION) as address:
        uri = RESOURCE_URIS[1]
        encoded_uri = base64.b64encode(uri.encode()).decode()
        message = request(
            "resources/read",
            params={"_meta": modern_meta(), "uri": uri},
        )

        status, _, body = post_json(
            address,
            message,
            modern_headers(
                "resources/read",
                name=f"=?base64?{encoded_uri}?=",
            ),
        )

        assert status == 200
        assert body["result"]["contents"][0]["uri"] == uri


def test_modern_http_streams_subscription_then_closes_gracefully() -> None:
    with running_http_server(MODERN_VERSION) as address:
        message = request(
            "subscriptions/listen",
            request_id=77,
            params={
                "_meta": modern_meta(),
                "notifications": {"toolsListChanged": True},
            },
        )
        connection = http.client.HTTPConnection(*address, timeout=2)
        connection.request(
            "POST",
            "/mcp",
            body=json.dumps(message, separators=(",", ":")),
            headers={
                "Content-Type": "application/json",
                **modern_headers("subscriptions/listen"),
            },
        )

        response = connection.getresponse()
        body = response.read().decode()
        connection.close()
        events = [
            json.loads(block.removeprefix("data: "))
            for block in body.strip().split("\n\n")
        ]

        assert response.status == 200
        assert response.getheader("Content-Type") == "text/event-stream"
        assert events[0]["method"] == "notifications/subscriptions/acknowledged"
        assert events[1]["method"] == "notifications/tools/list_changed"
        assert events[-1]["id"] == 77
        assert events[-1]["result"]["resultType"] == "complete"


def test_modern_http_rejects_removed_methods_and_unsupported_versions() -> None:
    with running_http_server(MODERN_VERSION) as address:
        message = request(
            "tools/list",
            params={"_meta": modern_meta(version=LEGACY_VERSION)},
        )
        headers = modern_headers("tools/list")
        headers["MCP-Protocol-Version"] = LEGACY_VERSION

        status, _, body = post_json(address, message, headers)
        assert status == 400
        assert body["error"]["code"] == UNSUPPORTED_PROTOCOL_VERSION

        connection = http.client.HTTPConnection(*address, timeout=2)
        connection.request("GET", "/mcp")
        get_response = connection.getresponse()
        get_response.read()
        assert get_response.status == 405

        connection.request("DELETE", "/mcp")
        delete_response = connection.getresponse()
        delete_response.read()
        connection.close()
        assert delete_response.status == 405


def test_modern_http_accepts_notification_without_request_headers() -> None:
    with running_http_server(MODERN_VERSION) as address:
        status, _, body = post_json(
            address,
            {
                "jsonrpc": "2.0",
                "method": "notifications/cancelled",
                "params": {"requestId": 1, "reason": "fixture test"},
            },
            {},
        )

        assert status == 202
        assert body is None


def test_modern_http_rejects_untrusted_origin() -> None:
    with running_http_server(MODERN_VERSION) as address:
        status, _, body = post_json(
            address,
            request("server/discover"),
            {
                **modern_headers("server/discover"),
                "Origin": "https://attacker.example",
            },
        )

        assert status == 403
        assert body["error"]["message"] == "Origin is not allowed"


def test_legacy_http_mints_and_terminates_session() -> None:
    with running_http_server(LEGACY_VERSION) as address:
        initialize = request(
            "initialize",
            params={
                "protocolVersion": LEGACY_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "legacy", "version": "1.0.0"},
            },
        )
        status, headers, body = post_json(address, initialize, {})
        assert status == 200
        assert body["result"]["protocolVersion"] == LEGACY_VERSION
        session_id = headers["mcp-session-id"]

        initialized = {
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {},
        }
        status, _, body = post_json(
            address, initialized, {"Mcp-Session-Id": session_id}
        )
        assert status == 202
        assert body is None

        status, _, body = post_json(
            address,
            request("tools/list", request_id=2, params={}),
            {"Mcp-Session-Id": session_id},
        )
        assert status == 200
        assert "resultType" not in body["result"]

        connection = http.client.HTTPConnection(*address, timeout=2)
        connection.request(
            "DELETE",
            "/mcp",
            headers={"Mcp-Session-Id": session_id},
        )
        response = connection.getresponse()
        response.read()
        connection.close()
        assert response.status == 200


def test_shipping_legacy_http_negotiates_and_retains_a_real_legacy_session() -> None:
    with running_http_server(SHIPPING_LEGACY_VERSION) as address:
        status, headers, body = post_json(
            address,
            request(
                "initialize",
                params={
                    "protocolVersion": SHIPPING_LEGACY_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "shipping-legacy", "version": "1.0.0"},
                },
            ),
            {},
        )

        assert status == 200
        assert body is not None
        assert body["result"]["protocolVersion"] == "2025-06-18"
        session_id = headers["mcp-session-id"]

        status, _, body = post_json(
            address,
            {
                "jsonrpc": "2.0",
                "method": "notifications/initialized",
                "params": {},
            },
            {"Mcp-Session-Id": session_id},
        )

        assert status == 202
        assert body is None

        status, _, body = post_json(
            address,
            request("tools/list", request_id=2, params={}),
            {"Mcp-Session-Id": session_id},
        )

        assert status == 200
        assert body is not None
        assert [tool["name"] for tool in body["result"]["tools"]] == [
            "client_metadata",
            "echo",
            "fail",
            "header_echo",
            "progress",
        ]
        assert "resultType" not in body["result"]


def test_review_http_lists_tools_across_two_pages() -> None:
    with running_http_server(MODERN_VERSION, profile=REVIEW_PROFILE) as address:
        status, _, first_body = post_json(
            address,
            request("tools/list"),
            modern_headers("tools/list"),
        )

        assert status == 200
        assert first_body is not None
        first = first_body["result"]
        assert [tool["name"] for tool in first["tools"]] == ["client_metadata", "echo"]
        assert first["nextCursor"] == REVIEW_PAGE_CURSOR

        status, _, second_body = post_json(
            address,
            request(
                "tools/list",
                request_id=2,
                params={"_meta": modern_meta(), "cursor": REVIEW_PAGE_CURSOR},
            ),
            modern_headers("tools/list"),
        )

        assert status == 200
        assert second_body is not None
        assert any(
            tool["name"] == "review_large_integer"
            for tool in second_body["result"]["tools"]
        )
        assert "nextCursor" not in second_body["result"]


def test_review_http_preserves_integer_precision_in_schema_and_tool_results() -> None:
    with running_http_server(MODERN_VERSION, profile=REVIEW_PROFILE) as address:
        status, _, tools_body = post_json(
            address,
            request(
                "tools/list",
                params={"_meta": modern_meta(), "cursor": REVIEW_PAGE_CURSOR},
            ),
            modern_headers("tools/list"),
        )

        assert status == 200
        assert tools_body is not None
        definition = next(
            tool
            for tool in tools_body["result"]["tools"]
            if tool["name"] == "review_large_integer"
        )
        assert definition["inputSchema"]["properties"]["value"]["minimum"] == (
            9_007_199_254_740_993
        )

        status, _, call_body = post_json(
            address,
            request(
                "tools/call",
                params={
                    "_meta": modern_meta(),
                    "name": "review_large_integer",
                    "arguments": {"value": REVIEW_EXACT_INTEGER},
                },
            ),
            modern_headers("tools/call", name="review_large_integer"),
        )

        assert status == 200
        assert call_body is not None
        assert call_body["result"]["structuredContent"] == {
            "value": 9_007_199_254_740_993
        }


def test_review_http_preserves_exact_integer_in_elicitation_form_schema() -> None:
    with running_http_server(MODERN_VERSION, profile=REVIEW_PROFILE) as address:
        status, _, body = post_json(
            address,
            request(
                "tools/call",
                params={
                    "_meta": modern_meta(capabilities={"elicitation": {"form": {}}}),
                    "name": "review_integer_elicitation",
                    "arguments": {},
                },
            ),
            modern_headers("tools/call", name="review_integer_elicitation"),
        )

    assert status == 200
    assert body is not None
    pending = body["result"]["inputRequests"]["large_integer"]
    assert pending["params"]["requestedSchema"]["properties"]["value"] == {
        "type": "integer",
        "minimum": 9_007_199_254_740_993,
        "maximum": 9_223_372_036_854_775_807,
        "default": 9_007_199_254_740_993,
    }


def test_review_http_returns_mismatched_discovery_response_id() -> None:
    with running_http_server(
        MODERN_VERSION,
        profile=MISMATCHED_DISCOVERY_ID_PROFILE,
    ) as address:
        status, _, body = post_json(
            address,
            request("server/discover", request_id=42),
            modern_headers("server/discover"),
        )

    assert status == 200
    assert body is not None
    assert body["id"] == "review-mismatched-42"


def test_review_http_returns_null_discovery_response_id() -> None:
    with running_http_server(
        MODERN_VERSION, profile=NULL_DISCOVERY_ID_PROFILE
    ) as address:
        status, _, body = post_json(
            address,
            request("server/discover", request_id=42),
            modern_headers("server/discover"),
        )

    assert status == 200
    assert body is not None
    assert body["id"] is None


def test_review_http_streams_sse_with_carriage_returns_and_comments() -> None:
    with running_http_server(
        MODERN_VERSION, profile=SSE_CR_COMMENTS_PROFILE
    ) as address:
        message = request(
            "subscriptions/listen",
            request_id=77,
            params={
                "_meta": modern_meta(),
                "notifications": {"toolsListChanged": True},
            },
        )
        connection = http.client.HTTPConnection(*address, timeout=2)
        connection.request(
            "POST",
            "/mcp",
            body=json.dumps(message, separators=(",", ":")),
            headers={
                "Content-Type": "application/json",
                **modern_headers("subscriptions/listen"),
            },
        )

        response = connection.getresponse()
        body = response.read()
        connection.close()

    assert response.status == 200
    assert response.getheader("Content-Type") == "text/event-stream"
    assert body.startswith(b": reviewer keepalive\rdata: ")
    assert b"\n" not in body
    events = [
        json.loads(line.removeprefix(b"data: "))
        for line in body.split(b"\r")
        if line.startswith(b"data: ")
    ]
    assert [event.get("method") for event in events[:-1]] == [
        "notifications/subscriptions/acknowledged",
        "notifications/tools/list_changed",
    ]
    assert events[-1]["id"] == 77
    assert events[-1]["result"]["resultType"] == "complete"


def test_review_http_streams_bounded_keepalive_flood_before_valid_sse_events() -> None:
    with running_http_server(
        MODERN_VERSION, profile=SSE_COMMENT_FLOOD_PROFILE
    ) as address:
        message = request(
            "subscriptions/listen",
            request_id=77,
            params={
                "_meta": modern_meta(),
                "notifications": {"toolsListChanged": True},
            },
        )
        connection = http.client.HTTPConnection(*address, timeout=10)
        connection.request(
            "POST",
            "/mcp",
            body=json.dumps(message, separators=(",", ":")),
            headers={
                "Content-Type": "application/json",
                **modern_headers("subscriptions/listen"),
            },
        )

        response = connection.getresponse()
        body = response.read()
        connection.close()

    assert response.status == 200
    assert response.getheader("Content-Type") == "text/event-stream"
    assert response.getheader("Content-Length") == str(len(body))
    assert (
        REVIEW_SSE_EVENT_LIMIT_BYTES
        < len(body)
        < (REVIEW_SSE_EVENT_LIMIT_BYTES + REVIEW_SSE_COMMENT_LINE_BYTES + 4_096)
    )
    assert body.count(b": reviewer keepalive ") == REVIEW_SSE_COMMENT_LINE_COUNT
    assert b"\n" not in body
    completed = json.loads(body.rsplit(b"data: ", 1)[1].removesuffix(b"\r\r"))
    assert completed["id"] == 77
    assert completed["result"]["resultType"] == "complete"
