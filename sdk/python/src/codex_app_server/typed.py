from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(slots=True)
class EmptyResult:
    @classmethod
    def from_dict(cls, payload: dict[str, Any] | None = None) -> "EmptyResult":
        return cls()


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
class ThreadForkResult:
    thread: ThreadRef

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ThreadForkResult":
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
class TurnSteerResult:
    turn_id: str

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "TurnSteerResult":
        return cls(turn_id=str(payload.get("turnId", "")))


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
class ItemLifecycleEvent:
    thread_id: str
    turn_id: str
    item: dict[str, Any]

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ItemLifecycleEvent":
        return cls(
            thread_id=str(payload.get("threadId", "")),
            turn_id=str(payload.get("turnId", "")),
            item=dict(payload.get("item") or {}),
        )


@dataclass(slots=True)
class ThreadNameUpdatedEvent:
    thread_id: str
    thread_name: str | None

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ThreadNameUpdatedEvent":
        return cls(
            thread_id=str(payload.get("threadId", "")),
            thread_name=None if payload.get("threadName") is None else str(payload.get("threadName")),
        )


@dataclass(slots=True)
class TokenUsageBreakdown:
    cached_input_tokens: int
    input_tokens: int
    output_tokens: int
    reasoning_output_tokens: int
    total_tokens: int

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "TokenUsageBreakdown":
        return cls(
            cached_input_tokens=int(payload.get("cachedInputTokens", 0) or 0),
            input_tokens=int(payload.get("inputTokens", 0) or 0),
            output_tokens=int(payload.get("outputTokens", 0) or 0),
            reasoning_output_tokens=int(payload.get("reasoningOutputTokens", 0) or 0),
            total_tokens=int(payload.get("totalTokens", 0) or 0),
        )


@dataclass(slots=True)
class ThreadTokenUsage:
    last: TokenUsageBreakdown
    total: TokenUsageBreakdown
    model_context_window: int | None = None

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ThreadTokenUsage":
        return cls(
            last=TokenUsageBreakdown.from_dict(payload.get("last") or {}),
            total=TokenUsageBreakdown.from_dict(payload.get("total") or {}),
            model_context_window=None
            if payload.get("modelContextWindow") is None
            else int(payload.get("modelContextWindow")),
        )


@dataclass(slots=True)
class ThreadTokenUsageUpdatedEvent:
    thread_id: str
    turn_id: str
    token_usage: ThreadTokenUsage

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ThreadTokenUsageUpdatedEvent":
        return cls(
            thread_id=str(payload.get("threadId", "")),
            turn_id=str(payload.get("turnId", "")),
            token_usage=ThreadTokenUsage.from_dict(payload.get("tokenUsage") or {}),
        )


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
