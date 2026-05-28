from __future__ import annotations

from typing import Any


class CodexError(Exception):
    """Base exception for SDK errors."""


class JsonRpcError(CodexError):
    """Raw JSON-RPC error wrapper from the server."""

    def __init__(self, code: int, message: str, data: Any = None):
        super().__init__(f"JSON-RPC error {code}: {message}")
        self.code = code
        self.message = message
        self.data = data


class TransportClosedError(CodexError):
    """Raised when the Codex transport closes unexpectedly."""


class CodexRpcError(JsonRpcError):
    """Base typed error for JSON-RPC failures."""


class ParseError(CodexRpcError):
    """Raised when a request or response cannot be parsed."""


class InvalidRequestError(CodexRpcError):
    """Raised when the runtime rejects the request shape."""


class MethodNotFoundError(CodexRpcError):
    """Raised when the requested operation is unavailable."""


class InvalidParamsError(CodexRpcError):
    """Raised when an operation receives invalid parameters."""


class InternalRpcError(CodexRpcError):
    """Raised when the runtime reports an internal RPC failure."""


class ServerBusyError(CodexRpcError):
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
            for value in direct.values():
                if isinstance(value, str) and value.lower() == "server_overloaded":
                    return True
        for value in data.values():
            if _is_server_overloaded(value):
                return True

    if isinstance(data, list):
        return any(_is_server_overloaded(value) for value in data)

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

    if -32099 <= code <= -32000:
        if _is_server_overloaded(data):
            if _contains_retry_limit_text(message):
                return RetryLimitExceededError(code, message, data)
            return ServerBusyError(code, message, data)
        if _contains_retry_limit_text(message):
            return RetryLimitExceededError(code, message, data)
        return CodexRpcError(code, message, data)

    return JsonRpcError(code, message, data)


def is_retryable_error(exc: BaseException) -> bool:
    """True if the exception is a transient overload-style error."""

    if isinstance(exc, ServerBusyError):
        return True

    if isinstance(exc, JsonRpcError):
        return _is_server_overloaded(exc.data)

    return False
