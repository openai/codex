from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal, Mapping, TypeAlias

from .items import ThreadItem, parse_thread_item


@dataclass(slots=True)
class ThreadStartedEvent:
    thread_id: str
    type: Literal["thread.started"] = "thread.started"


@dataclass(slots=True)
class TurnStartedEvent:
    type: Literal["turn.started"] = "turn.started"


@dataclass(slots=True)
class Usage:
    input_tokens: int
    cached_input_tokens: int
    output_tokens: int


@dataclass(slots=True)
class TurnCompletedEvent:
    usage: Usage
    type: Literal["turn.completed"] = "turn.completed"


@dataclass(slots=True)
class ThreadError:
    message: str


@dataclass(slots=True)
class TurnFailedEvent:
    error: ThreadError
    type: Literal["turn.failed"] = "turn.failed"


@dataclass(slots=True)
class ItemStartedEvent:
    item: ThreadItem
    type: Literal["item.started"] = "item.started"


@dataclass(slots=True)
class ItemUpdatedEvent:
    item: ThreadItem
    type: Literal["item.updated"] = "item.updated"


@dataclass(slots=True)
class ItemCompletedEvent:
    item: ThreadItem
    type: Literal["item.completed"] = "item.completed"


@dataclass(slots=True)
class ThreadErrorEvent:
    message: str
    type: Literal["error"] = "error"


ThreadEvent: TypeAlias = (
    ThreadStartedEvent
    | TurnStartedEvent
    | TurnCompletedEvent
    | TurnFailedEvent
    | ItemStartedEvent
    | ItemUpdatedEvent
    | ItemCompletedEvent
    | ThreadErrorEvent
)


def parse_thread_event(raw: Mapping[str, Any]) -> ThreadEvent:
    event_type = str(raw.get("type", ""))

    if event_type == "thread.started":
        return ThreadStartedEvent(thread_id=str(raw.get("thread_id", "")))
    if event_type == "turn.started":
        return TurnStartedEvent()
    if event_type == "turn.completed":
        usage = raw.get("usage")
        usage_obj: Mapping[str, Any]
        if isinstance(usage, Mapping):
            usage_obj = usage
        else:
            usage_obj = {}
        return TurnCompletedEvent(
            usage=Usage(
                input_tokens=int(usage_obj.get("input_tokens", 0)),
                cached_input_tokens=int(usage_obj.get("cached_input_tokens", 0)),
                output_tokens=int(usage_obj.get("output_tokens", 0)),
            )
        )
    if event_type == "turn.failed":
        raw_error = raw.get("error")
        if isinstance(raw_error, Mapping):
            message = str(raw_error.get("message", ""))
        else:
            message = str(raw_error or "")
        return TurnFailedEvent(error=ThreadError(message=message))
    if event_type == "item.started":
        item = raw.get("item")
        if not isinstance(item, Mapping):
            raise ValueError("item.started event missing item")
        return ItemStartedEvent(item=parse_thread_item(item))
    if event_type == "item.updated":
        item = raw.get("item")
        if not isinstance(item, Mapping):
            raise ValueError("item.updated event missing item")
        return ItemUpdatedEvent(item=parse_thread_item(item))
    if event_type == "item.completed":
        item = raw.get("item")
        if not isinstance(item, Mapping):
            raise ValueError("item.completed event missing item")
        return ItemCompletedEvent(item=parse_thread_item(item))
    if event_type == "error":
        return ThreadErrorEvent(message=str(raw.get("message", "")))

    raise ValueError(f"Unknown thread event type: {event_type}")
