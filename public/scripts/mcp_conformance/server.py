#!/usr/bin/env python3
"""MCP test server for shipping and draft legacy protocols and MCP 2026-07-28."""

import argparse
import base64
import binascii
import json
import os
import sys
import threading
import uuid
from dataclasses import dataclass, field
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import IO, Mapping, Sequence
from urllib.parse import urlsplit

SHIPPING_LEGACY_VERSION = "2025-06-18"
LEGACY_VERSION = "2025-11-25"
MODERN_VERSION = "2026-07-28"
SERVER_NAME = "openai-mcp-spec-test-server"
SERVER_VERSION = "0.1.0"

DEFAULT_PROFILE = "default"
REVIEW_PROFILE = "review-regressions"
REPEATED_CURSOR_PROFILE = "repeated-cursor"
MISMATCHED_DISCOVERY_ID_PROFILE = "discovery-mismatched-id"
NULL_DISCOVERY_ID_PROFILE = "discovery-null-id"
SSE_CR_COMMENTS_PROFILE = "sse-cr-comments"
SSE_COMMENT_FLOOD_PROFILE = "sse-comment-flood"
CATALOG_MAX_PROFILE = "catalog-max"
CATALOG_OVER_LIMIT_PROFILE = "catalog-over-limit"
FIXTURE_PROFILES = (
    DEFAULT_PROFILE,
    REVIEW_PROFILE,
    REPEATED_CURSOR_PROFILE,
    MISMATCHED_DISCOVERY_ID_PROFILE,
    NULL_DISCOVERY_ID_PROFILE,
    SSE_CR_COMMENTS_PROFILE,
    SSE_COMMENT_FLOOD_PROFILE,
    CATALOG_MAX_PROFILE,
    CATALOG_OVER_LIMIT_PROFILE,
)
MAX_CATALOG_ITEMS = 1_024
REVIEW_EXACT_INTEGER = 9_007_199_254_740_993
REVIEW_MAX_INTEGER = 9_223_372_036_854_775_807
REVIEW_MRTR_INPUT_REQUEST_COUNT = 65
REVIEW_PAGE_CURSOR = "review-page-2"
REVIEW_SSE_EVENT_LIMIT_BYTES = 8 * 1_024 * 1_024
REVIEW_SSE_COMMENT_LINE_BYTES = 2_048
REVIEW_SSE_COMMENT_LINE_COUNT = (
    REVIEW_SSE_EVENT_LIMIT_BYTES // REVIEW_SSE_COMMENT_LINE_BYTES + 1
)

PARSE_ERROR = -32700
INVALID_REQUEST = -32600
METHOD_NOT_FOUND = -32601
INVALID_PARAMS = -32602
INTERNAL_ERROR = -32603
HEADER_MISMATCH = -32020
MISSING_REQUIRED_CLIENT_CAPABILITY = -32021
UNSUPPORTED_PROTOCOL_VERSION = -32022

SERVER_INFO_META = {
    "io.modelcontextprotocol/serverInfo": {
        "name": SERVER_NAME,
        "version": SERVER_VERSION,
    }
}

RESOURCE_URIS = (
    "test://fixture/readme",
    "test://fixture/こんにちは",
)


@dataclass
class ConnectionState:
    initialize_seen: bool = False
    initialized: bool = False


@dataclass
class ResponsePlan:
    response: dict[str, object] | None
    notifications: list[dict[str, object]] = field(default_factory=list)
    http_status: int = HTTPStatus.OK
    force_sse: bool = False


class HeaderValidationError(ValueError):
    pass


def _error_response(
    request_id: object,
    code: int,
    message: str,
    *,
    data: dict[str, object] | None = None,
) -> dict[str, object]:
    error: dict[str, object] = {"code": code, "message": message}
    if data is not None:
        error["data"] = data
    return {"jsonrpc": "2.0", "id": request_id, "error": error}


def _response(request_id: object, result: dict[str, object]) -> dict[str, object]:
    return {"jsonrpc": "2.0", "id": request_id, "result": result}


def _modern_result(
    fields: Mapping[str, object] | None = None,
    *,
    cache_scope: str | None = None,
    ttl_ms: int | None = None,
    meta: Mapping[str, object] | None = None,
) -> dict[str, object]:
    result: dict[str, object] = {
        "resultType": "complete",
        "_meta": dict(meta or SERVER_INFO_META),
    }
    if fields is not None:
        result.update(fields)
    if cache_scope is not None:
        result["cacheScope"] = cache_scope
    if ttl_ms is not None:
        result["ttlMs"] = ttl_ms
    return result


def _tool_definitions(
    modern: bool,
    *,
    profile: str = DEFAULT_PROFILE,
) -> list[dict[str, object]]:
    definitions: list[dict[str, object]] = [
        {
            "name": "client_metadata",
            "description": "Return the per-request MCP client metadata observed by the server.",
            "inputSchema": {"type": "object", "additionalProperties": False},
            "outputSchema": {"type": "object"},
            "annotations": {"readOnlyHint": True},
        },
        {
            "name": "echo",
            "description": "Echo text through a normal MCP tool result.",
            "inputSchema": {
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"],
                "additionalProperties": False,
            },
            "outputSchema": {
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"],
                "additionalProperties": False,
            },
            "annotations": {"readOnlyHint": True},
        },
        {
            "name": "fail",
            "description": "Return a tool execution error without using a JSON-RPC error.",
            "inputSchema": {"type": "object", "additionalProperties": False},
            "annotations": {"readOnlyHint": True},
        },
        {
            "name": "header_echo",
            "description": "Echo values that modern HTTP clients must mirror into headers.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "region": {"type": "string", "x-mcp-header": "Region"},
                    "attempt": {
                        "type": "integer",
                        "minimum": -9_007_199_254_740_991,
                        "maximum": 9_007_199_254_740_991,
                        "x-mcp-header": "Attempt",
                    },
                    "enabled": {"type": "boolean", "x-mcp-header": "Enabled"},
                    "greeting": {"type": "string", "x-mcp-header": "Greeting"},
                },
                "required": ["region", "attempt", "enabled", "greeting"],
                "additionalProperties": False,
            },
            "outputSchema": {"type": "object"},
            "annotations": {"readOnlyHint": True},
        },
        {
            "name": "progress",
            "description": "Emit request-scoped progress and opt-in log notifications.",
            "inputSchema": {"type": "object", "additionalProperties": False},
            "annotations": {"readOnlyHint": True},
        },
    ]
    if modern:
        # This tool first returns InputRequiredResult, not CallToolResult. Keep
        # output-schema coverage on echo so the MRTR fixture tests one concern.
        definitions.append(
            {
                "name": "request_input",
                "description": "Exercise the 2026-07-28 multi round-trip request pattern.",
                "inputSchema": {"type": "object", "additionalProperties": False},
                "annotations": {"readOnlyHint": True},
            }
        )
    if profile in (REVIEW_PROFILE, REPEATED_CURSOR_PROFILE):
        definitions.extend(
            [
                {
                    "name": "review_large_integer",
                    "description": "Echo an integer without losing JSON-schema precision.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "value": {
                                "type": "integer",
                                "minimum": REVIEW_EXACT_INTEGER,
                                "maximum": REVIEW_MAX_INTEGER,
                                "default": REVIEW_EXACT_INTEGER,
                            }
                        },
                        "required": ["value"],
                        "additionalProperties": False,
                    },
                    "outputSchema": {
                        "type": "object",
                        "properties": {"value": {"type": "integer"}},
                        "required": ["value"],
                        "additionalProperties": False,
                    },
                    "annotations": {"readOnlyHint": True},
                },
                {
                    "name": "review_protocol_env",
                    "description": "Report the MCP protocol-version environment variable.",
                    "inputSchema": {"type": "object", "additionalProperties": False},
                    "outputSchema": {
                        "type": "object",
                        "properties": {"value": {"type": ["string", "null"]}},
                        "required": ["value"],
                        "additionalProperties": False,
                    },
                    "annotations": {"readOnlyHint": True},
                },
            ]
        )
        if modern:
            definitions.extend(
                [
                    {
                        "name": "review_integer_elicitation",
                        "description": "Request an integer form without losing schema precision.",
                        "inputSchema": {
                            "type": "object",
                            "additionalProperties": False,
                        },
                        "annotations": {"readOnlyHint": True},
                    },
                    {
                        "name": "review_mrtr_cap",
                        "description": (
                            "Return more simultaneous input requests than the client cap."
                        ),
                        "inputSchema": {
                            "type": "object",
                            "additionalProperties": False,
                        },
                        "annotations": {"readOnlyHint": True},
                    },
                ]
            )
    if profile in (CATALOG_MAX_PROFILE, CATALOG_OVER_LIMIT_PROFILE):
        catalog_size = MAX_CATALOG_ITEMS
        if profile == CATALOG_OVER_LIMIT_PROFILE:
            catalog_size += 1
        definitions.extend(
            {
                "name": f"catalog_boundary_{index:04d}",
                "description": "Exercise the MCP catalog item boundary.",
                "inputSchema": {
                    "type": "object",
                    "additionalProperties": False,
                },
                "annotations": {"readOnlyHint": True},
            }
            for index in range(catalog_size - len(definitions))
        )
    return sorted(definitions, key=lambda tool: str(tool["name"]))


class ProtocolServer:
    def __init__(self, mode: str, *, profile: str = DEFAULT_PROFILE) -> None:
        if mode not in (SHIPPING_LEGACY_VERSION, LEGACY_VERSION, MODERN_VERSION):
            raise ValueError(f"unsupported mode: {mode}")
        if profile not in FIXTURE_PROFILES:
            raise ValueError(f"unsupported profile: {profile}")
        self.mode = mode
        self.profile = profile

    @property
    def modern(self) -> bool:
        return self.mode == MODERN_VERSION

    def handle(
        self,
        message: object,
        state: ConnectionState,
    ) -> ResponsePlan:
        if not isinstance(message, dict):
            return ResponsePlan(
                _error_response(None, INVALID_REQUEST, "Request must be a JSON object"),
                http_status=HTTPStatus.BAD_REQUEST,
            )

        if message.get("jsonrpc") != "2.0" or not isinstance(
            message.get("method"), str
        ):
            return ResponsePlan(
                _error_response(
                    message.get("id"),
                    INVALID_REQUEST,
                    "Invalid JSON-RPC 2.0 request",
                ),
                http_status=HTTPStatus.BAD_REQUEST,
            )

        method = str(message["method"])
        is_notification = "id" not in message
        request_id = message.get("id")
        params = message.get("params", {})
        if not isinstance(params, dict):
            return ResponsePlan(
                None
                if is_notification
                else _error_response(
                    request_id, INVALID_PARAMS, "params must be an object"
                ),
                http_status=HTTPStatus.BAD_REQUEST,
            )

        if is_notification:
            return self._handle_notification(method, params, state)

        if self.modern:
            return self._handle_modern(request_id, method, params)
        return self._handle_legacy(request_id, method, params, state)

    def _handle_notification(
        self,
        method: str,
        params: dict[str, object],
        state: ConnectionState,
    ) -> ResponsePlan:
        if not self.modern and method == "notifications/initialized":
            if state.initialize_seen:
                state.initialized = True
            return ResponsePlan(None, http_status=HTTPStatus.ACCEPTED)
        if method == "notifications/cancelled":
            return ResponsePlan(None, http_status=HTTPStatus.ACCEPTED)
        return ResponsePlan(None, http_status=HTTPStatus.ACCEPTED)

    def _handle_modern(
        self,
        request_id: object,
        method: str,
        params: dict[str, object],
    ) -> ResponsePlan:
        if method == "initialize":
            return ResponsePlan(
                _error_response(
                    request_id,
                    METHOD_NOT_FOUND,
                    f"initialize is not supported; this server speaks MCP {MODERN_VERSION}",
                    data={"supported": [MODERN_VERSION]},
                ),
                http_status=HTTPStatus.NOT_FOUND,
            )

        meta = params.get("_meta")
        if not isinstance(meta, dict):
            return ResponsePlan(
                _error_response(
                    request_id,
                    INVALID_PARAMS,
                    "params._meta is required on every 2026-07-28 request",
                ),
                http_status=HTTPStatus.BAD_REQUEST,
            )

        version = meta.get("io.modelcontextprotocol/protocolVersion")
        capabilities = meta.get("io.modelcontextprotocol/clientCapabilities")
        if not isinstance(version, str) or not isinstance(capabilities, dict):
            return ResponsePlan(
                _error_response(
                    request_id,
                    INVALID_PARAMS,
                    "_meta must include protocolVersion and clientCapabilities",
                ),
                http_status=HTTPStatus.BAD_REQUEST,
            )
        if version != MODERN_VERSION:
            return ResponsePlan(
                _error_response(
                    request_id,
                    UNSUPPORTED_PROTOCOL_VERSION,
                    "Unsupported protocol version",
                    data={"supported": [MODERN_VERSION], "requested": version},
                ),
                http_status=HTTPStatus.BAD_REQUEST,
            )

        if method == "server/discover":
            response_id = request_id
            if self.profile == MISMATCHED_DISCOVERY_ID_PROFILE:
                response_id = f"review-mismatched-{request_id}"
            elif self.profile == NULL_DISCOVERY_ID_PROFILE:
                response_id = None
            return ResponsePlan(
                _response(
                    response_id,
                    _modern_result(
                        {
                            "supportedVersions": [MODERN_VERSION],
                            "capabilities": self._modern_capabilities(),
                            "instructions": (
                                "Use echo for a basic call, header_echo for HTTP "
                                "header mirroring, request_input for MRTR, and "
                                "progress for request-scoped notifications."
                            ),
                        },
                        cache_scope="public",
                        ttl_ms=60_000,
                    ),
                )
            )
        if method == "tools/list":
            return self._list_tools(request_id, params)
        if method == "tools/call":
            return self._call_tool(request_id, params, meta)
        if method == "resources/list":
            return self._list_resources(request_id, params)
        if method == "resources/templates/list":
            return ResponsePlan(
                _response(
                    request_id,
                    _modern_result(
                        {
                            "resourceTemplates": [
                                {
                                    "name": "fixture-by-name",
                                    "uriTemplate": "test://fixture/{name}",
                                    "description": "Read a named fixture resource.",
                                    "mimeType": "text/plain",
                                }
                            ]
                        },
                        cache_scope="public",
                        ttl_ms=5_000,
                    ),
                )
            )
        if method == "resources/read":
            return self._read_resource(request_id, params)
        if method == "prompts/list":
            return ResponsePlan(
                _response(
                    request_id,
                    _modern_result(
                        {"prompts": self._prompts()},
                        cache_scope="public",
                        ttl_ms=5_000,
                    ),
                )
            )
        if method == "prompts/get":
            return self._get_prompt(request_id, params)
        if method == "subscriptions/listen":
            return self._listen(request_id, params)

        return ResponsePlan(
            _error_response(request_id, METHOD_NOT_FOUND, f"Unknown method: {method}"),
            http_status=HTTPStatus.NOT_FOUND,
        )

    def _handle_legacy(
        self,
        request_id: object,
        method: str,
        params: dict[str, object],
        state: ConnectionState,
    ) -> ResponsePlan:
        if method == "initialize":
            requested_version = params.get("protocolVersion")
            if not isinstance(requested_version, str):
                return ResponsePlan(
                    _error_response(
                        request_id,
                        INVALID_PARAMS,
                        "initialize requires protocolVersion",
                    )
                )
            state.initialize_seen = True
            return ResponsePlan(
                _response(
                    request_id,
                    {
                        "protocolVersion": self.mode,
                        "capabilities": self._legacy_capabilities(),
                        "serverInfo": {
                            "name": SERVER_NAME,
                            "version": SERVER_VERSION,
                        },
                        "instructions": f"Legacy {self.mode} compatibility fixture.",
                    },
                )
            )

        if method == "server/discover":
            return ResponsePlan(
                _error_response(
                    request_id,
                    METHOD_NOT_FOUND,
                    "Unknown method: server/discover",
                ),
                http_status=HTTPStatus.NOT_FOUND,
            )
        if not state.initialized:
            return ResponsePlan(
                _error_response(
                    request_id,
                    INVALID_REQUEST,
                    "Server has not received notifications/initialized",
                )
            )
        if method == "ping":
            return ResponsePlan(_response(request_id, {}))
        if method == "logging/setLevel":
            return ResponsePlan(_response(request_id, {}))
        if method == "tools/list":
            return self._list_tools(request_id, params)
        if method == "tools/call":
            return self._call_tool(request_id, params, {})
        if method == "resources/list":
            return self._list_resources(request_id, params)
        if method == "resources/templates/list":
            return ResponsePlan(
                _response(
                    request_id,
                    {
                        "resourceTemplates": [
                            {
                                "name": "fixture-by-name",
                                "uriTemplate": "test://fixture/{name}",
                                "description": "Read a named fixture resource.",
                                "mimeType": "text/plain",
                            }
                        ]
                    },
                )
            )
        if method == "resources/read":
            return self._read_resource(request_id, params)
        if method == "prompts/list":
            return ResponsePlan(_response(request_id, {"prompts": self._prompts()}))
        if method == "prompts/get":
            return self._get_prompt(request_id, params)
        return ResponsePlan(
            _error_response(request_id, METHOD_NOT_FOUND, f"Unknown method: {method}"),
            http_status=HTTPStatus.NOT_FOUND,
        )

    def _list_tools(
        self,
        request_id: object,
        params: dict[str, object],
    ) -> ResponsePlan:
        definitions = _tool_definitions(self.modern, profile=self.profile)
        fields: dict[str, object]
        if self.modern and self.profile in (REVIEW_PROFILE, REPEATED_CURSOR_PROFILE):
            cursor = params.get("cursor")
            if cursor is None:
                fields = {
                    "tools": definitions[:2],
                    "nextCursor": REVIEW_PAGE_CURSOR,
                }
            elif cursor == REVIEW_PAGE_CURSOR:
                fields = {"tools": definitions[2:]}
                if self.profile == REPEATED_CURSOR_PROFILE:
                    fields["nextCursor"] = REVIEW_PAGE_CURSOR
            else:
                return ResponsePlan(
                    _error_response(request_id, INVALID_PARAMS, "Invalid tool cursor")
                )
        else:
            fields = {"tools": definitions}

        result = (
            _modern_result(fields, cache_scope="public", ttl_ms=1_000)
            if self.modern
            else fields
        )
        return ResponsePlan(_response(request_id, result))

    @staticmethod
    def _modern_capabilities() -> dict[str, object]:
        return {
            "tools": {"listChanged": True},
            "resources": {"subscribe": True, "listChanged": True},
            "prompts": {"listChanged": True},
            "logging": {},
            "extensions": {"com.openai/mcp-spec-test": {}},
        }

    @staticmethod
    def _legacy_capabilities() -> dict[str, object]:
        return {
            "tools": {"listChanged": True},
            "resources": {"subscribe": True, "listChanged": True},
            "prompts": {"listChanged": True},
            "logging": {},
        }

    def _call_tool(
        self,
        request_id: object,
        params: dict[str, object],
        request_meta: dict[str, object],
    ) -> ResponsePlan:
        name = params.get("name")
        arguments = params.get("arguments", {})
        if not isinstance(name, str) or not isinstance(arguments, dict):
            return ResponsePlan(
                _error_response(
                    request_id,
                    INVALID_PARAMS,
                    "tools/call requires a string name and object arguments",
                )
            )
        known = {
            str(tool["name"])
            for tool in _tool_definitions(self.modern, profile=self.profile)
        }
        if name not in known:
            return ResponsePlan(
                _error_response(request_id, INVALID_PARAMS, f"Unknown tool: {name}")
            )

        if name == "request_input":
            return self._request_input(request_id, params, request_meta)
        if name == "review_integer_elicitation":
            return self._review_integer_elicitation(request_id, params, request_meta)
        if name == "review_mrtr_cap":
            return self._review_mrtr_cap(request_id, request_meta)

        notifications: list[dict[str, object]] = []
        if name == "progress":
            progress_token = request_meta.get("progressToken")
            if isinstance(progress_token, (str, int)) and not isinstance(
                progress_token, bool
            ):
                notifications.append(
                    {
                        "jsonrpc": "2.0",
                        "method": "notifications/progress",
                        "params": {
                            "progressToken": progress_token,
                            "progress": 1,
                            "total": 1,
                            "message": "fixture complete",
                        },
                    }
                )
            if isinstance(request_meta.get("io.modelcontextprotocol/logLevel"), str):
                notifications.append(
                    {
                        "jsonrpc": "2.0",
                        "method": "notifications/message",
                        "params": {
                            "level": "info",
                            "logger": SERVER_NAME,
                            "data": "request-scoped fixture log",
                        },
                    }
                )

        if name == "review_large_integer":
            value = arguments.get("value")
            if (
                not isinstance(value, int)
                or isinstance(value, bool)
                or not REVIEW_EXACT_INTEGER <= value <= REVIEW_MAX_INTEGER
            ):
                return ResponsePlan(
                    _error_response(
                        request_id,
                        INVALID_PARAMS,
                        "review_large_integer requires an exactly representable JSON integer",
                    )
                )
            structured = {"value": value}
            fields: dict[str, object] = {
                "content": [{"type": "text", "text": json.dumps(structured)}],
                "structuredContent": structured,
            }
        elif name == "review_protocol_env":
            structured = {"value": os.environ.get("CODEX_MCP_PROTOCOL_VERSION")}
            fields = {
                "content": [{"type": "text", "text": json.dumps(structured)}],
                "structuredContent": structured,
            }
        elif name == "echo":
            text = arguments.get("text")
            if not isinstance(text, str):
                return ResponsePlan(
                    _error_response(
                        request_id,
                        INVALID_PARAMS,
                        "echo requires a string text argument",
                    )
                )
            fields = {
                "content": [{"type": "text", "text": text}],
                "structuredContent": {"text": text},
            }
        elif name == "client_metadata":
            fields = {
                "content": [
                    {
                        "type": "text",
                        "text": json.dumps(request_meta, sort_keys=True),
                    }
                ],
                "structuredContent": request_meta,
            }
        elif name == "header_echo":
            required = {
                "region": str,
                "attempt": int,
                "enabled": bool,
                "greeting": str,
            }
            if any(
                key not in arguments
                or not isinstance(arguments[key], expected_type)
                or (expected_type is int and isinstance(arguments[key], bool))
                for key, expected_type in required.items()
            ) or not (
                -9_007_199_254_740_991
                <= int(arguments["attempt"])
                <= 9_007_199_254_740_991
            ):
                return ResponsePlan(
                    _error_response(
                        request_id,
                        INVALID_PARAMS,
                        "header_echo arguments have invalid types",
                    )
                )
            fields = {
                "content": [
                    {
                        "type": "text",
                        "text": json.dumps(
                            arguments, ensure_ascii=False, sort_keys=True
                        ),
                    }
                ],
                "structuredContent": arguments,
            }
        elif name == "fail":
            fields = {
                "content": [
                    {
                        "type": "text",
                        "text": "Intentional tool execution failure",
                    }
                ],
                "isError": True,
            }
        else:
            fields = {
                "content": [{"type": "text", "text": "progress complete"}],
            }

        result = _modern_result(fields) if self.modern else fields
        return ResponsePlan(
            _response(request_id, result),
            notifications=notifications,
            force_sse=bool(notifications),
        )

    def _review_integer_elicitation(
        self,
        request_id: object,
        params: dict[str, object],
        request_meta: dict[str, object],
    ) -> ResponsePlan:
        capabilities = request_meta.get(
            "io.modelcontextprotocol/clientCapabilities", {}
        )
        if not isinstance(capabilities, dict) or not isinstance(
            capabilities.get("elicitation"), dict
        ):
            return ResponsePlan(
                _error_response(
                    request_id,
                    MISSING_REQUIRED_CLIENT_CAPABILITY,
                    "review_integer_elicitation requires elicitation support",
                    data={"requiredCapabilities": {"elicitation": {"form": {}}}},
                ),
                http_status=HTTPStatus.BAD_REQUEST,
            )

        input_responses = params.get("inputResponses")
        request_state = params.get("requestState")
        expected_state = "opaque:review_integer_elicitation:v1"
        if input_responses is None and request_state is None:
            return ResponsePlan(
                _response(
                    request_id,
                    {
                        "resultType": "input_required",
                        "_meta": dict(SERVER_INFO_META),
                        "inputRequests": {
                            "large_integer": {
                                "method": "elicitation/create",
                                "params": {
                                    "mode": "form",
                                    "message": "Return the exact large integer.",
                                    "requestedSchema": {
                                        "type": "object",
                                        "properties": {
                                            "value": {
                                                "type": "integer",
                                                "minimum": REVIEW_EXACT_INTEGER,
                                                "maximum": REVIEW_MAX_INTEGER,
                                                "default": REVIEW_EXACT_INTEGER,
                                            }
                                        },
                                        "required": ["value"],
                                    },
                                },
                            }
                        },
                        "requestState": expected_state,
                    },
                )
            )

        if request_state != expected_state or not isinstance(input_responses, dict):
            return ResponsePlan(
                _error_response(
                    request_id,
                    INVALID_PARAMS,
                    "integer elicitation retry must echo requestState and inputResponses",
                )
            )

        response = input_responses.get("large_integer")
        if not isinstance(response, dict) or response.get("action") != "accept":
            return ResponsePlan(
                _error_response(
                    request_id, INVALID_PARAMS, "integer input must be accepted"
                )
            )

        content = response.get("content")
        value = content.get("value") if isinstance(content, dict) else None
        if (
            not isinstance(value, int)
            or isinstance(value, bool)
            or not REVIEW_EXACT_INTEGER <= value <= REVIEW_MAX_INTEGER
        ):
            return ResponsePlan(
                _error_response(
                    request_id,
                    INVALID_PARAMS,
                    "integer elicitation requires the exact integer form response",
                )
            )

        return ResponsePlan(
            _response(
                request_id,
                _modern_result(
                    {
                        "content": [{"type": "text", "text": str(value)}],
                        "structuredContent": {"value": value},
                    }
                ),
            )
        )

    def _review_mrtr_cap(
        self,
        request_id: object,
        request_meta: dict[str, object],
    ) -> ResponsePlan:
        capabilities = request_meta.get(
            "io.modelcontextprotocol/clientCapabilities", {}
        )
        if not isinstance(capabilities, dict) or not isinstance(
            capabilities.get("elicitation"), dict
        ):
            return ResponsePlan(
                _error_response(
                    request_id,
                    MISSING_REQUIRED_CLIENT_CAPABILITY,
                    "review_mrtr_cap requires elicitation support",
                    data={"requiredCapabilities": {"elicitation": {"form": {}}}},
                ),
                http_status=HTTPStatus.BAD_REQUEST,
            )

        return ResponsePlan(
            _response(
                request_id,
                {
                    "resultType": "input_required",
                    "_meta": dict(SERVER_INFO_META),
                    "inputRequests": {
                        f"review-input-{index:02d}": {
                            "method": "elicitation/create",
                            "params": {
                                "mode": "form",
                                "message": f"Review MRTR input {index}.",
                                "requestedSchema": {
                                    "type": "object",
                                    "properties": {"value": {"type": "string"}},
                                    "required": ["value"],
                                },
                            },
                        }
                        for index in range(REVIEW_MRTR_INPUT_REQUEST_COUNT)
                    },
                    "requestState": "opaque:review_mrtr_cap:v1",
                },
            )
        )

    def _request_input(
        self,
        request_id: object,
        params: dict[str, object],
        request_meta: dict[str, object],
    ) -> ResponsePlan:
        capabilities = request_meta.get(
            "io.modelcontextprotocol/clientCapabilities", {}
        )
        if not isinstance(capabilities, dict) or not isinstance(
            capabilities.get("elicitation"), dict
        ):
            return ResponsePlan(
                _error_response(
                    request_id,
                    MISSING_REQUIRED_CLIENT_CAPABILITY,
                    "request_input requires elicitation support",
                    data={"requiredCapabilities": {"elicitation": {"form": {}}}},
                ),
                http_status=HTTPStatus.BAD_REQUEST,
            )

        input_responses = params.get("inputResponses")
        request_state = params.get("requestState")
        expected_state = "opaque:request_input:v1"
        if input_responses is None and request_state is None:
            return ResponsePlan(
                _response(
                    request_id,
                    {
                        "resultType": "input_required",
                        "_meta": dict(SERVER_INFO_META),
                        "inputRequests": {
                            "confirmation": {
                                "method": "elicitation/create",
                                "params": {
                                    "mode": "form",
                                    "message": "Confirm the MCP 2026-07-28 MRTR test.",
                                    "requestedSchema": {
                                        "type": "object",
                                        "properties": {
                                            "confirmation": {
                                                "type": "string",
                                                "default": "confirmed",
                                            }
                                        },
                                        "required": ["confirmation"],
                                    },
                                },
                            }
                        },
                        "requestState": expected_state,
                    },
                )
            )

        if request_state != expected_state or not isinstance(input_responses, dict):
            return ResponsePlan(
                _error_response(
                    request_id,
                    INVALID_PARAMS,
                    "MRTR retry must echo requestState and provide inputResponses",
                )
            )
        confirmation = input_responses.get("confirmation")
        if not isinstance(confirmation, dict) or confirmation.get("action") != "accept":
            return ResponsePlan(
                _error_response(
                    request_id,
                    INVALID_PARAMS,
                    "confirmation input must be accepted",
                )
            )
        content = confirmation.get("content")
        if not isinstance(content, dict) or not isinstance(
            content.get("confirmation"), str
        ):
            return ResponsePlan(
                _error_response(
                    request_id,
                    INVALID_PARAMS,
                    "confirmation content is required",
                )
            )
        value = str(content["confirmation"])
        return ResponsePlan(
            _response(
                request_id,
                _modern_result(
                    {
                        "content": [{"type": "text", "text": value}],
                        "structuredContent": {"confirmation": value},
                    }
                ),
            )
        )

    def _list_resources(
        self,
        request_id: object,
        params: dict[str, object],
    ) -> ResponsePlan:
        cursor = params.get("cursor")
        if cursor is None:
            resources = [self._resource(RESOURCE_URIS[0])]
            next_cursor: str | None = "page-2"
        elif cursor == "page-2":
            resources = [self._resource(RESOURCE_URIS[1])]
            next_cursor = None
        else:
            return ResponsePlan(
                _error_response(request_id, INVALID_PARAMS, "Invalid resource cursor")
            )

        fields: dict[str, object] = {"resources": resources}
        if next_cursor is not None:
            fields["nextCursor"] = next_cursor
        result = (
            _modern_result(fields, cache_scope="public", ttl_ms=2_000)
            if self.modern
            else fields
        )
        return ResponsePlan(_response(request_id, result))

    @staticmethod
    def _resource(uri: str) -> dict[str, object]:
        return {
            "name": uri.rsplit("/", 1)[-1],
            "title": f"Fixture resource: {uri.rsplit('/', 1)[-1]}",
            "uri": uri,
            "description": "A deterministic text resource from the MCP spec fixture.",
            "mimeType": "text/plain",
        }

    def _read_resource(
        self,
        request_id: object,
        params: dict[str, object],
    ) -> ResponsePlan:
        uri = params.get("uri")
        if uri not in RESOURCE_URIS:
            return ResponsePlan(
                _error_response(request_id, INVALID_PARAMS, "Resource not found")
            )
        fields: dict[str, object] = {
            "contents": [
                {
                    "uri": uri,
                    "mimeType": "text/plain",
                    "text": f"fixture contents for {uri}",
                }
            ]
        }
        result = (
            _modern_result(fields, cache_scope="private", ttl_ms=500)
            if self.modern
            else fields
        )
        return ResponsePlan(_response(request_id, result))

    @staticmethod
    def _prompts() -> list[dict[str, object]]:
        return [
            {
                "name": "fixture.greeting",
                "title": "Fixture greeting",
                "description": "Create a deterministic greeting.",
                "arguments": [
                    {
                        "name": "name",
                        "description": "Name to greet.",
                        "required": True,
                    }
                ],
            }
        ]

    def _get_prompt(
        self,
        request_id: object,
        params: dict[str, object],
    ) -> ResponsePlan:
        if params.get("name") != "fixture.greeting":
            return ResponsePlan(
                _error_response(request_id, INVALID_PARAMS, "Unknown prompt")
            )
        arguments = params.get("arguments", {})
        if not isinstance(arguments, dict) or not isinstance(
            arguments.get("name"), str
        ):
            return ResponsePlan(
                _error_response(
                    request_id,
                    INVALID_PARAMS,
                    "fixture.greeting requires the name argument",
                )
            )
        fields: dict[str, object] = {
            "description": "A deterministic fixture greeting.",
            "messages": [
                {
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": f"Hello, {arguments['name']}!",
                    },
                }
            ],
        }
        result = _modern_result(fields) if self.modern else fields
        return ResponsePlan(_response(request_id, result))

    def _listen(
        self,
        request_id: object,
        params: dict[str, object],
    ) -> ResponsePlan:
        requested = params.get("notifications")
        if not isinstance(requested, dict):
            return ResponsePlan(
                _error_response(
                    request_id,
                    INVALID_PARAMS,
                    "subscriptions/listen requires a notifications filter",
                )
            )

        acknowledged: dict[str, object] = {}
        if requested.get("toolsListChanged") is True:
            acknowledged["toolsListChanged"] = True
        if requested.get("promptsListChanged") is True:
            acknowledged["promptsListChanged"] = True
        if requested.get("resourcesListChanged") is True:
            acknowledged["resourcesListChanged"] = True
        subscriptions = requested.get("resourceSubscriptions")
        valid_subscriptions = (
            [uri for uri in subscriptions if uri in RESOURCE_URIS]
            if isinstance(subscriptions, list)
            and all(isinstance(uri, str) for uri in subscriptions)
            else []
        )
        if valid_subscriptions:
            acknowledged["resourceSubscriptions"] = valid_subscriptions

        subscription_meta = {
            "io.modelcontextprotocol/subscriptionId": request_id,
        }
        notifications: list[dict[str, object]] = [
            {
                "jsonrpc": "2.0",
                "method": "notifications/subscriptions/acknowledged",
                "params": {
                    "_meta": subscription_meta,
                    "notifications": acknowledged,
                },
            }
        ]
        notification_methods = (
            ("toolsListChanged", "notifications/tools/list_changed"),
            ("promptsListChanged", "notifications/prompts/list_changed"),
            ("resourcesListChanged", "notifications/resources/list_changed"),
        )
        for filter_name, method in notification_methods:
            if acknowledged.get(filter_name) is True:
                notifications.append(
                    {
                        "jsonrpc": "2.0",
                        "method": method,
                        "params": {"_meta": subscription_meta},
                    }
                )
        for uri in valid_subscriptions:
            notifications.append(
                {
                    "jsonrpc": "2.0",
                    "method": "notifications/resources/updated",
                    "params": {"_meta": subscription_meta, "uri": uri},
                }
            )

        closing_meta = {
            **SERVER_INFO_META,
            "io.modelcontextprotocol/subscriptionId": request_id,
        }
        return ResponsePlan(
            _response(
                request_id,
                _modern_result(meta=closing_meta),
            ),
            notifications=notifications,
            force_sse=True,
        )


def _decode_mirrored_header(value: str) -> str:
    if value.startswith("=?base64?") and value.endswith("?="):
        encoded = value[len("=?base64?") : -len("?=")]
        try:
            return base64.b64decode(encoded, validate=True).decode("utf-8")
        except (binascii.Error, UnicodeDecodeError) as exc:
            raise HeaderValidationError("invalid Base64 mirrored header") from exc
    if value.startswith("=?base64?") or value.endswith("?="):
        raise HeaderValidationError("malformed Base64 sentinel")
    if value != value.strip() or any(
        char != "\t" and (ord(char) < 0x20 or ord(char) > 0x7E) for char in value
    ):
        raise HeaderValidationError("unsafe plain mirrored header")
    return value


def validate_modern_http_headers(
    message: dict[str, object],
    headers: Mapping[str, str],
) -> None:
    normalized = {name.lower(): value for name, value in headers.items()}
    accept = {
        value.strip().split(";", 1)[0].lower()
        for value in normalized.get("accept", "").split(",")
    }
    if not {"application/json", "text/event-stream"}.issubset(accept):
        raise HeaderValidationError(
            "Accept must include application/json and text/event-stream"
        )

    method = message.get("method")
    params = message.get("params")
    if not isinstance(method, str) or not isinstance(params, dict):
        return
    meta = params.get("_meta")

    header_version = normalized.get("mcp-protocol-version")
    if header_version is None:
        raise HeaderValidationError("missing MCP-Protocol-Version header")
    if isinstance(meta, dict):
        body_version = meta.get("io.modelcontextprotocol/protocolVersion")
        if isinstance(body_version, str) and body_version != header_version:
            raise HeaderValidationError(
                "MCP-Protocol-Version header does not match request _meta"
            )

    header_method = normalized.get("mcp-method")
    if header_method is None:
        raise HeaderValidationError("missing Mcp-Method header")
    if header_method != method:
        raise HeaderValidationError("Mcp-Method header does not match request method")

    name_source: object | None = None
    if method in ("tools/call", "prompts/get"):
        name_source = params.get("name")
    elif method == "resources/read":
        name_source = params.get("uri")
    if name_source is not None:
        header_name = normalized.get("mcp-name")
        if header_name is None:
            raise HeaderValidationError("missing Mcp-Name header")
        if (
            not isinstance(name_source, str)
            or _decode_mirrored_header(header_name) != name_source
        ):
            raise HeaderValidationError("Mcp-Name header does not match request body")

    if method != "tools/call" or params.get("name") != "header_echo":
        return
    arguments = params.get("arguments", {})
    if not isinstance(arguments, dict):
        return
    mirrored = {
        "region": "region",
        "attempt": "attempt",
        "enabled": "enabled",
        "greeting": "greeting",
    }
    for argument_name, header_suffix in mirrored.items():
        header_key = f"mcp-param-{header_suffix}"
        argument = arguments.get(argument_name)
        header_value = normalized.get(header_key)
        if argument is None:
            if header_value is not None:
                raise HeaderValidationError(
                    f"Mcp-Param-{header_suffix} must be omitted"
                )
            continue
        if header_value is None:
            raise HeaderValidationError(f"missing Mcp-Param-{header_suffix} header")
        decoded = _decode_mirrored_header(header_value)
        if isinstance(argument, bool):
            expected = "true" if argument else "false"
        elif isinstance(argument, int):
            expected = str(argument)
        elif isinstance(argument, str):
            expected = argument
        else:
            raise HeaderValidationError(
                f"unsupported mirrored argument type for {argument_name}"
            )
        if decoded != expected:
            raise HeaderValidationError(
                f"Mcp-Param-{header_suffix} header does not match request body"
            )


def _encode_sse_messages(
    messages: Sequence[Mapping[str, object]],
    *,
    profile: str,
) -> bytes:
    if profile == SSE_COMMENT_FLOOD_PROFILE:
        comment_prefix = b": reviewer keepalive "
        comment = (
            comment_prefix
            + b"x" * (REVIEW_SSE_COMMENT_LINE_BYTES - len(comment_prefix) - 1)
            + b"\r"
        )
        prefix = comment * REVIEW_SSE_COMMENT_LINE_COUNT
        separator = "\r\r"
    elif profile == SSE_CR_COMMENTS_PROFILE:
        prefix = b": reviewer keepalive\r"
        separator = "\r\r"
    else:
        prefix = b""
        separator = "\n\n"

    return prefix + b"".join(
        (
            f"data: {json.dumps(message, ensure_ascii=False, separators=(',', ':'))}"
            f"{separator}"
        ).encode()
        for message in messages
    )


class FixtureHTTPServer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(
        self,
        address: tuple[str, int],
        fixture: ProtocolServer,
        *,
        endpoint: str,
        allowed_origins: Sequence[str],
        log_requests: bool,
    ) -> None:
        self.fixture = fixture
        self.endpoint = endpoint
        self.allowed_origins = set(allowed_origins)
        self.log_requests = log_requests
        self.sessions: dict[str, ConnectionState] = {}
        self.sessions_lock = threading.Lock()
        super().__init__(address, FixtureRequestHandler)


class FixtureRequestHandler(BaseHTTPRequestHandler):
    server: FixtureHTTPServer

    def do_GET(self) -> None:
        if self.path == "/healthz":
            body = json.dumps(
                {"status": "ok", "mode": self.server.fixture.mode}
            ).encode()
            self._send_bytes(HTTPStatus.OK, "application/json", body)
            return
        self._send_bytes(
            HTTPStatus.METHOD_NOT_ALLOWED,
            "application/json",
            b"",
            extra_headers={"Allow": "POST"},
        )

    def do_DELETE(self) -> None:
        if self.path != self.server.endpoint or self.server.fixture.modern:
            self._send_bytes(
                HTTPStatus.METHOD_NOT_ALLOWED,
                "application/json",
                b"",
                extra_headers={"Allow": "POST"},
            )
            return
        session_id = self.headers.get("Mcp-Session-Id")
        with self.server.sessions_lock:
            existed = (
                session_id is not None
                and self.server.sessions.pop(session_id, None) is not None
            )
        self._send_bytes(
            HTTPStatus.OK if existed else HTTPStatus.NOT_FOUND,
            "application/json",
            b"",
        )

    def do_POST(self) -> None:
        if self.path != self.server.endpoint:
            self._send_json_error(
                None,
                HTTPStatus.NOT_FOUND,
                METHOD_NOT_FOUND,
                f"Unknown MCP endpoint: {self.path}",
            )
            return
        if not self._origin_is_allowed():
            self._send_json_error(
                None,
                HTTPStatus.FORBIDDEN,
                INVALID_REQUEST,
                "Origin is not allowed",
            )
            return
        if self.headers.get_content_type() != "application/json":
            self._send_json_error(
                None,
                HTTPStatus.UNSUPPORTED_MEDIA_TYPE,
                INVALID_REQUEST,
                "Content-Type must be application/json",
            )
            return
        try:
            content_length = int(self.headers.get("Content-Length", ""))
        except ValueError:
            content_length = -1
        if content_length < 0 or content_length > 1_048_576:
            self._send_json_error(
                None,
                HTTPStatus.BAD_REQUEST,
                INVALID_REQUEST,
                "Invalid Content-Length",
            )
            return
        try:
            message = json.loads(self.rfile.read(content_length))
        except (json.JSONDecodeError, UnicodeDecodeError):
            self._send_json_error(
                None,
                HTTPStatus.BAD_REQUEST,
                PARSE_ERROR,
                "Parse error",
            )
            return
        if not isinstance(message, dict):
            self._send_json_error(
                None,
                HTTPStatus.BAD_REQUEST,
                INVALID_REQUEST,
                "Request must be a JSON object",
            )
            return

        request_id = message.get("id")
        if self.server.fixture.modern:
            if "id" in message:
                try:
                    validate_modern_http_headers(message, self.headers)
                except HeaderValidationError as exc:
                    self._send_json_error(
                        request_id,
                        HTTPStatus.BAD_REQUEST,
                        HEADER_MISMATCH,
                        f"Header mismatch: {exc}",
                    )
                    return
            state = ConnectionState()
            response_headers: dict[str, str] = {}
        else:
            state, response_headers = self._legacy_state(message)
            if state is None:
                return

        plan = self.server.fixture.handle(message, state)
        if plan.response is None:
            self._send_bytes(
                plan.http_status,
                "application/json",
                b"",
                extra_headers=response_headers,
            )
            return
        if plan.force_sse:
            messages = [*plan.notifications, plan.response]
            body = _encode_sse_messages(messages, profile=self.server.fixture.profile)
            self._send_bytes(
                plan.http_status,
                "text/event-stream",
                body,
                extra_headers={
                    **response_headers,
                    "Cache-Control": "no-cache",
                    "X-Accel-Buffering": "no",
                },
            )
            return
        body = json.dumps(
            plan.response, ensure_ascii=False, separators=(",", ":")
        ).encode()
        self._send_bytes(
            plan.http_status,
            "application/json",
            body,
            extra_headers=response_headers,
        )

    def _legacy_state(
        self,
        message: dict[str, object],
    ) -> tuple[ConnectionState | None, dict[str, str]]:
        method = message.get("method")
        if method == "initialize":
            state = ConnectionState()
            session_id = uuid.uuid4().hex
            with self.server.sessions_lock:
                self.server.sessions[session_id] = state
            return state, {"Mcp-Session-Id": session_id}
        if method == "server/discover":
            return ConnectionState(), {}
        session_id = self.headers.get("Mcp-Session-Id")
        with self.server.sessions_lock:
            state = (
                self.server.sessions.get(session_id) if session_id is not None else None
            )
        if state is None:
            self._send_json_error(
                message.get("id"),
                HTTPStatus.BAD_REQUEST,
                INVALID_REQUEST,
                "Missing or unknown Mcp-Session-Id",
            )
            return None, {}
        return state, {"Mcp-Session-Id": session_id}

    def _origin_is_allowed(self) -> bool:
        origin = self.headers.get("Origin")
        if origin is None:
            return True
        if origin in self.server.allowed_origins:
            return True
        parsed = urlsplit(origin)
        return parsed.scheme in ("http", "https") and parsed.hostname in (
            "localhost",
            "127.0.0.1",
            "::1",
        )

    def _send_json_error(
        self,
        request_id: object,
        status: int,
        code: int,
        message: str,
    ) -> None:
        body = json.dumps(
            _error_response(request_id, code, message), separators=(",", ":")
        ).encode()
        self._send_bytes(status, "application/json", body)

    def _send_bytes(
        self,
        status: int,
        content_type: str,
        body: bytes,
        *,
        extra_headers: Mapping[str, str] | None = None,
    ) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        if extra_headers is not None:
            for name, value in extra_headers.items():
                self.send_header(name, value)
        self.end_headers()
        if body:
            self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:
        if not self.server.log_requests:
            return
        print(
            f"{self.address_string()} - {format % args}",
            file=sys.stderr,
            flush=True,
        )


def make_http_server(
    fixture: ProtocolServer,
    host: str,
    port: int,
    *,
    endpoint: str = "/mcp",
    allowed_origins: Sequence[str] = (),
    log_requests: bool = True,
) -> FixtureHTTPServer:
    return FixtureHTTPServer(
        (host, port),
        fixture,
        endpoint=endpoint,
        allowed_origins=allowed_origins,
        log_requests=log_requests,
    )


def run_stdio(
    fixture: ProtocolServer,
    stdin: IO[str],
    stdout: IO[str],
) -> None:
    state = ConnectionState()
    for line in stdin:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            stdout.write(
                json.dumps(
                    _error_response(None, PARSE_ERROR, "Parse error"),
                    separators=(",", ":"),
                )
                + "\n"
            )
            stdout.flush()
            continue
        plan = fixture.handle(message, state)
        for notification in plan.notifications:
            stdout.write(
                json.dumps(notification, ensure_ascii=False, separators=(",", ":"))
                + "\n"
            )
        if plan.response is not None:
            stdout.write(
                json.dumps(plan.response, ensure_ascii=False, separators=(",", ":"))
                + "\n"
            )
        stdout.flush()


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run a deterministic MCP server for testing shipping 2025-06-18, "
            "2025-11-25 compatibility, or 2026-07-28 draft compliance."
        )
    )
    parser.add_argument(
        "--mode",
        choices=(SHIPPING_LEGACY_VERSION, LEGACY_VERSION, MODERN_VERSION),
        default=MODERN_VERSION,
    )
    parser.add_argument(
        "--transport",
        choices=("stdio", "http"),
        default="stdio",
    )
    parser.add_argument(
        "--profile",
        choices=FIXTURE_PROFILES,
        default=DEFAULT_PROFILE,
        help="Select an optional deterministic reviewer-regression fixture profile.",
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--endpoint", default="/mcp")
    parser.add_argument(
        "--allowed-origin",
        action="append",
        default=[],
        help="Additional exact Origin value accepted by the HTTP transport.",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    fixture = ProtocolServer(args.mode, profile=args.profile)
    if args.transport == "stdio":
        print(
            f"{SERVER_NAME} starting stdio mode={args.mode}",
            file=sys.stderr,
            flush=True,
        )
        run_stdio(fixture, sys.stdin, sys.stdout)
        return 0

    server = make_http_server(
        fixture,
        args.host,
        args.port,
        endpoint=args.endpoint,
        allowed_origins=args.allowed_origin,
    )
    print(
        f"{SERVER_NAME} listening on http://{args.host}:{args.port}{args.endpoint} "
        f"mode={args.mode}",
        file=sys.stderr,
        flush=True,
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
