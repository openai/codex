class CodexError(Exception):
    """Base exception for the Codex SDK."""


class CodexNotInstalledError(CodexError):
    """Raised when the Codex CLI executable cannot be found."""


class AuthRequiredError(CodexError):
    """Raised when neither ChatGPT auth nor API key is available."""


class AbortError(CodexError):
    """Raised when a turn is aborted by a signal."""


class ThreadRunError(CodexError):
    """Raised when the Codex CLI execution fails."""
