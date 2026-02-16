from __future__ import annotations

from dataclasses import dataclass
from typing import Any


class AppServerError(Exception):
    """Base exception for SDK errors."""


@dataclass(slots=True)
class JsonRpcErrorPayload:
    code: int
    message: str
    data: Any = None


class JsonRpcError(AppServerError):
    """Raw JSON-RPC error wrapper from the server."""

    def __init__(self, code: int, message: str, data: Any = None):
        super().__init__(f"JSON-RPC error {code}: {message}")
        self.code = code
        self.message = message
        self.data = data


class TransportClosedError(AppServerError):
    """Raised when the app-server transport closes unexpectedly."""


class AppServerRpcError(JsonRpcError):
    """Base typed error for JSON-RPC failures."""


class ParseError(AppServerRpcError):
    pass


class InvalidRequestError(AppServerRpcError):
    pass


class MethodNotFoundError(AppServerRpcError):
    pass


class InvalidParamsError(AppServerRpcError):
    pass


class InternalRpcError(AppServerRpcError):
    pass


class ServerBusyError(AppServerRpcError):
    """Server is overloaded / unavailable and caller should retry."""


class RetryLimitExceededError(ServerBusyError):
    """Server exhausted internal retry budget for a retryable operation."""


def _contains_retry_limit_text(message: str) -> bool:
    lowered = message.lower()
    return "retry limit" in lowered or "too many failed attempts" in lowered


def _is_server_overloaded(data: Any) -> bool:
    if data is None:
        return False

    if isinstance(data, str):
        return data.lower() == "server_overloaded"

    if isinstance(data, dict):
        direct = data.get("codex_error_info") or data.get("codexErrorInfo") or data.get("errorInfo")
        if isinstance(direct, str) and direct.lower() == "server_overloaded":
            return True
        if isinstance(direct, dict):
            for v in direct.values():
                if isinstance(v, str) and v.lower() == "server_overloaded":
                    return True
        for v in data.values():
            if _is_server_overloaded(v):
                return True

    if isinstance(data, list):
        return any(_is_server_overloaded(v) for v in data)

    return False


def map_jsonrpc_error(code: int, message: str, data: Any = None) -> JsonRpcError:
    """Map a raw JSON-RPC error into a richer SDK exception class."""

    if code == -32700:
        return ParseError(code, message, data)
    if code == -32600:
        return InvalidRequestError(code, message, data)
    if code == -32601:
        return MethodNotFoundError(code, message, data)
    if code == -32602:
        return InvalidParamsError(code, message, data)
    if code == -32603:
        return InternalRpcError(code, message, data)

    # Reserved server-error range in JSON-RPC 2.0.
    if -32099 <= code <= -32000:
        if _is_server_overloaded(data):
            if _contains_retry_limit_text(message):
                return RetryLimitExceededError(code, message, data)
            return ServerBusyError(code, message, data)
        if _contains_retry_limit_text(message):
            return RetryLimitExceededError(code, message, data)
        return AppServerRpcError(code, message, data)

    return JsonRpcError(code, message, data)


def is_retryable_error(exc: BaseException) -> bool:
    """True if the exception is a transient overload-style error."""

    if isinstance(exc, ServerBusyError):
        return True

    if isinstance(exc, JsonRpcError):
        return _is_server_overloaded(exc.data)

    return False
