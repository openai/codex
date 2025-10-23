from __future__ import annotations

from dataclasses import dataclass
from typing import Sequence


class CodexError(Exception):
    """Base exception for Codex SDK."""


def _format_command(command: Sequence[str] | None) -> str:
    if not command:
        return "<unknown>"
    return " ".join(command)


class UnsupportedPlatformError(CodexError):
    def __init__(self, platform: str, machine: str) -> None:
        message = f"Unsupported platform: {platform} ({machine})"
        super().__init__(message)
        self.platform = platform
        self.machine = machine


class SpawnError(CodexError):
    def __init__(self, command: Sequence[str] | None, error: OSError) -> None:
        self.command = list(command) if command else None
        self.original_error = error
        super().__init__(f"Failed to spawn codex exec: {_format_command(self.command)}: {error}")


@dataclass(slots=True)
class ExecExitError(CodexError):
    command: tuple[str, ...]
    exit_code: int
    stderr: str

    def __str__(self) -> str:  # pragma: no cover - trivial formatting
        stderr = self.stderr.strip()
        tail = f": {stderr}" if stderr else ""
        return f"codex exec exited with code {self.exit_code}{tail}"


@dataclass(slots=True)
class JsonParseError(CodexError):
    raw_line: str
    command: tuple[str, ...]

    def __str__(self) -> str:  # pragma: no cover - trivial formatting
        sample = self.raw_line
        if len(sample) > 200:
            sample = sample[:197] + "..."
        return f"Failed to parse codex event: {sample}"


class ThreadRunError(CodexError):
    def __init__(self, message: str) -> None:
        super().__init__(message)


class SchemaValidationError(CodexError):
    def __init__(self, message: str) -> None:
        super().__init__(message)
