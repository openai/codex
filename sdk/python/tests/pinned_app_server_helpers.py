from __future__ import annotations

from collections.abc import AsyncIterator, Iterable, Iterator
from typing import Any

from app_server_harness import (
    ev_assistant_message,
    ev_completed,
    ev_message_item_added,
    ev_output_text_delta,
    ev_response_created,
    sse,
)
from openai_codex.generated.v2_all import (
    AgentMessageDeltaNotification,
    ItemCompletedNotification,
    MessagePhase,
)
from openai_codex.models import Notification

TINY_PNG_BYTES = bytes(
    [
        137,
        80,
        78,
        71,
        13,
        10,
        26,
        10,
        0,
        0,
        0,
        13,
        73,
        72,
        68,
        82,
        0,
        0,
        0,
        1,
        0,
        0,
        0,
        1,
        8,
        6,
        0,
        0,
        0,
        31,
        21,
        196,
        137,
        0,
        0,
        0,
        11,
        73,
        68,
        65,
        84,
        120,
        156,
        99,
        96,
        0,
        2,
        0,
        0,
        5,
        0,
        1,
        122,
        94,
        171,
        63,
        0,
        0,
        0,
        0,
        73,
        69,
        78,
        68,
        174,
        66,
        96,
        130,
    ]
)


def response_approval_policy(response: Any) -> str:
    """Return serialized approvalPolicy from a generated thread response."""
    return response.model_dump(by_alias=True, mode="json")["approvalPolicy"]


def agent_message_texts(events: list[Notification]) -> list[str]:
    """Extract completed agent-message text from SDK notifications."""
    texts: list[str] = []
    for event in events:
        if not isinstance(event.payload, ItemCompletedNotification):
            continue
        item = event.payload.item.root
        if item.type == "agentMessage":
            texts.append(item.text)
    return texts


def agent_message_texts_from_items(items: Iterable[Any]) -> list[str]:
    """Extract agent-message text from completed run result items."""
    texts: list[str] = []
    for item in items:
        root = item.root
        if root.type == "agentMessage":
            texts.append(root.text)
    return texts


def next_sync_delta(stream: Iterator[Notification]) -> str:
    """Advance a sync turn stream until the next agent-message text delta."""
    for event in stream:
        if isinstance(event.payload, AgentMessageDeltaNotification):
            return event.payload.delta
    raise AssertionError("stream completed before an agent-message delta")


async def next_async_delta(stream: AsyncIterator[Notification]) -> str:
    """Advance an async turn stream until the next agent-message text delta."""
    async for event in stream:
        if isinstance(event.payload, AgentMessageDeltaNotification):
            return event.payload.delta
    raise AssertionError("stream completed before an agent-message delta")


def streaming_response(response_id: str, item_id: str, parts: list[str]) -> str:
    """Build an SSE stream with text deltas and a final assistant message."""
    return sse(
        [
            ev_response_created(response_id),
            ev_message_item_added(item_id),
            *[ev_output_text_delta(part) for part in parts],
            ev_assistant_message(item_id, "".join(parts)),
            ev_completed(response_id),
        ]
    )


def assistant_message_with_phase(
    item_id: str,
    text: str,
    phase: MessagePhase,
) -> dict[str, Any]:
    """Build an assistant message event carrying app-server phase metadata."""
    event = ev_assistant_message(item_id, text)
    event["item"] = {**event["item"], "phase": phase.value}
    return event


def request_kind(request_path: str) -> str:
    """Classify captured mock-server request paths for compact assertions."""
    if request_path.endswith("/responses/compact"):
        return "compact"
    if request_path.endswith("/responses"):
        return "responses"
    return request_path
