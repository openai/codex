from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(slots=True)
class ThreadRef:
    id: str
    preview: str = ""

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ThreadRef":
        return cls(id=str(payload["id"]), preview=str(payload.get("preview", "")))


@dataclass(slots=True)
class TurnRef:
    id: str
    status: str

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "TurnRef":
        return cls(id=str(payload["id"]), status=str(payload.get("status", "unknown")))


@dataclass(slots=True)
class ThreadStartResult:
    thread: ThreadRef

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ThreadStartResult":
        return cls(thread=ThreadRef.from_dict(payload["thread"]))


@dataclass(slots=True)
class TurnStartResult:
    turn: TurnRef

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "TurnStartResult":
        return cls(turn=TurnRef.from_dict(payload["turn"]))
