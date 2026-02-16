from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(slots=True)
class Notification:
    method: str
    params: dict[str, Any] | None


@dataclass(slots=True)
class RequestMessage:
    id: str | int
    method: str
    params: dict[str, Any] | None


@dataclass(slots=True)
class ResponseMessage:
    id: str | int
    result: Any


@dataclass(slots=True)
class AskResult:
    """High-level notebook-friendly response bundle for text turns."""

    thread_id: str
    text: str
    completed: Notification
