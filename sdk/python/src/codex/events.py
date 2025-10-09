from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal

from .exceptions import CodexError
from .items import ThreadItem, parse_thread_item


@dataclass(frozen=True, slots=True)
class Usage:
    input_tokens: int
    cached_input_tokens: int
    output_tokens: int


@dataclass(frozen=True, slots=True)
class ThreadError:
    message: str


@dataclass(frozen=True, slots=True)
class ThreadStartedEvent:
    type: Literal["thread.started"] = field(default="thread.started", init=False)
    thread_id: str


@dataclass(frozen=True, slots=True)
class TurnStartedEvent:
    type: Literal["turn.started"] = field(default="turn.started", init=False)


@dataclass(frozen=True, slots=True)
class TurnCompletedEvent:
    type: Literal["turn.completed"] = field(default="turn.completed", init=False)
    usage: Usage


@dataclass(frozen=True, slots=True)
class TurnFailedEvent:
    type: Literal["turn.failed"] = field(default="turn.failed", init=False)
    error: ThreadError


@dataclass(frozen=True, slots=True)
class ItemStartedEvent:
    type: Literal["item.started"] = field(default="item.started", init=False)
    item: ThreadItem


@dataclass(frozen=True, slots=True)
class ItemUpdatedEvent:
    type: Literal["item.updated"] = field(default="item.updated", init=False)
    item: ThreadItem


@dataclass(frozen=True, slots=True)
class ItemCompletedEvent:
    type: Literal["item.completed"] = field(default="item.completed", init=False)
    item: ThreadItem


@dataclass(frozen=True, slots=True)
class ThreadErrorEvent:
    type: Literal["error"] = field(default="error", init=False)
    message: str


ThreadEvent = (
    ThreadStartedEvent
    | TurnStartedEvent
    | TurnCompletedEvent
    | TurnFailedEvent
    | ItemStartedEvent
    | ItemUpdatedEvent
    | ItemCompletedEvent
    | ThreadErrorEvent
)


def _ensure_dict(payload: object) -> dict[str, object]:
    if isinstance(payload, dict):
        return payload
    raise CodexError("Event payload must be an object")


def _ensure_str(value: object, field: str) -> str:
    if isinstance(value, str):
        return value
    raise CodexError(f"Expected string for {field}")


def _ensure_int(value: object, field: str) -> int:
    if isinstance(value, int):
        return value
    raise CodexError(f"Expected integer for {field}")


def _parse_usage(payload: object) -> Usage:
    data = _ensure_dict(payload)
    return Usage(
        input_tokens=_ensure_int(data.get("input_tokens"), "input_tokens"),
        cached_input_tokens=_ensure_int(data.get("cached_input_tokens"), "cached_input_tokens"),
        output_tokens=_ensure_int(data.get("output_tokens"), "output_tokens"),
    )


def parse_thread_event(payload: object) -> ThreadEvent:
    data = _ensure_dict(payload)
    type_name = _ensure_str(data.get("type"), "type")

    if type_name == "thread.started":
        thread_id = _ensure_str(data.get("thread_id"), "thread_id")
        return ThreadStartedEvent(thread_id=thread_id)

    if type_name == "turn.started":
        return TurnStartedEvent()

    if type_name == "turn.completed":
        usage = _parse_usage(data.get("usage"))
        return TurnCompletedEvent(usage=usage)

    if type_name == "turn.failed":
        error_payload = _ensure_dict(data.get("error"))
        message = _ensure_str(error_payload.get("message"), "error.message")
        return TurnFailedEvent(error=ThreadError(message=message))

    if type_name in {"item.started", "item.updated", "item.completed"}:
        item_payload = data.get("item")
        item = parse_thread_item(item_payload)
        if type_name == "item.started":
            return ItemStartedEvent(item=item)
        if type_name == "item.updated":
            return ItemUpdatedEvent(item=item)
        return ItemCompletedEvent(item=item)

    if type_name == "error":
        message = _ensure_str(data.get("message"), "message")
        return ThreadErrorEvent(message=message)

    raise CodexError(f"Unsupported event type: {type_name}")
