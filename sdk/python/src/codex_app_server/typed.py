from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(slots=True)
class ThreadRef:
    id: str
    preview: str = ""

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ThreadRef":
        return cls(id=str(payload.get("id", "")), preview=str(payload.get("preview", "")))


@dataclass(slots=True)
class TurnRef:
    id: str
    status: str = "unknown"

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "TurnRef":
        return cls(id=str(payload.get("id", "")), status=str(payload.get("status", "unknown")))


@dataclass(slots=True)
class ThreadStartResult:
    thread: ThreadRef

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ThreadStartResult":
        return cls(thread=ThreadRef.from_dict(payload.get("thread") or {}))


@dataclass(slots=True)
class ThreadResumeResult:
    thread: ThreadRef

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ThreadResumeResult":
        return cls(thread=ThreadRef.from_dict(payload.get("thread") or {}))


@dataclass(slots=True)
class ThreadReadResult:
    thread: ThreadRef

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ThreadReadResult":
        return cls(thread=ThreadRef.from_dict(payload.get("thread") or {}))


@dataclass(slots=True)
class ThreadListResult:
    data: list[ThreadRef]
    next_cursor: str | None = None

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ThreadListResult":
        return cls(
            data=[ThreadRef.from_dict(item or {}) for item in (payload.get("data") or [])],
            next_cursor=None if payload.get("nextCursor") is None else str(payload.get("nextCursor")),
        )


@dataclass(slots=True)
class TurnStartResult:
    turn: TurnRef

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "TurnStartResult":
        return cls(turn=TurnRef.from_dict(payload.get("turn") or {}))


@dataclass(slots=True)
class ModelRef:
    id: str

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ModelRef":
        return cls(id=str(payload.get("id", "")))


@dataclass(slots=True)
class ModelListResult:
    data: list[ModelRef]

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ModelListResult":
        return cls(data=[ModelRef.from_dict(item or {}) for item in (payload.get("data") or [])])


@dataclass(slots=True)
class TurnCompletedEvent:
    thread_id: str
    turn: TurnRef

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "TurnCompletedEvent":
        return cls(
            thread_id=str(payload.get("threadId", "")),
            turn=TurnRef.from_dict(payload.get("turn") or {}),
        )


@dataclass(slots=True)
class TurnStartedEvent:
    turn: TurnRef

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "TurnStartedEvent":
        return cls(turn=TurnRef.from_dict(payload.get("turn") or {}))


@dataclass(slots=True)
class ThreadStartedEvent:
    thread: ThreadRef

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ThreadStartedEvent":
        return cls(thread=ThreadRef.from_dict(payload.get("thread") or {}))


@dataclass(slots=True)
class AgentMessageDeltaEvent:
    item_id: str
    delta: str

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "AgentMessageDeltaEvent":
        return cls(item_id=str(payload.get("itemId", "")), delta=str(payload.get("delta", "")))


@dataclass(slots=True)
class ErrorEvent:
    message: str
    will_retry: bool
    thread_id: str
    turn_id: str

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ErrorEvent":
        error = payload.get("error") or {}
        return cls(
            message=str(error.get("message", "")),
            will_retry=bool(payload.get("willRetry", False)),
            thread_id=str(payload.get("threadId", "")),
            turn_id=str(payload.get("turnId", "")),
        )
