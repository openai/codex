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
