"""Error types for Codex Agent SDK."""

from typing import Any


class CodexSDKError(Exception):
    """Base exception for all Codex SDK errors."""


class CLIConnectionError(CodexSDKError):
    """Raised when unable to connect to Codex."""


class CLINotFoundError(CLIConnectionError):
    """Raised when Codex is not found or not installed."""

    def __init__(
        self, message: str = "Codex not found", cli_path: str | None = None
    ):
        if cli_path:
            message = f"{message}: {cli_path}"
        super().__init__(message)


class ProcessError(CodexSDKError):
    """Raised when the CLI process fails."""

    def __init__(
        self, message: str, exit_code: int | None = None, stderr: str | None = None
    ):
        self.exit_code = exit_code
        self.stderr = stderr

        if exit_code is not None:
            message = f"{message} (exit code: {exit_code})"
        if stderr:
            message = f"{message}\nError output: {stderr}"

        super().__init__(message)


class CodexJSONDecodeError(CodexSDKError):
    """Raised when unable to decode JSON from CLI output."""

    def __init__(self, line: str, original_error: Exception):
        self.line = line
        self.original_error = original_error
        super().__init__(f"Failed to decode JSON: {line[:100]}...")


# Deprecated alias
CLIJSONDecodeError = CodexJSONDecodeError


class MessageParseError(CodexSDKError):
    """Raised when unable to parse a message from CLI output."""

    def __init__(self, message: str, data: dict[str, Any] | None = None):
        self.data = data
        super().__init__(message)
