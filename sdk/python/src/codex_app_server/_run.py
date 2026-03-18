from __future__ import annotations

from dataclasses import dataclass
from typing import AsyncIterator, Iterator

from .generated.v2_all import (
    AgentMessageThreadItem,
    ItemCompletedNotification,
    MessagePhase,
    ThreadItem,
    ThreadTokenUsage,
    ThreadTokenUsageUpdatedNotification,
    Turn as AppServerTurn,
    TurnCompletedNotification,
    TurnStatus,
)
from .models import Notification


@dataclass(slots=True)
class RunResult:
    final_response: str
    items: list[ThreadItem]
    usage: ThreadTokenUsage | None


def _agent_message_item_from_thread_item(
    item: ThreadItem,
) -> AgentMessageThreadItem | None:
    thread_item = item.root if hasattr(item, "root") else item
    if isinstance(thread_item, AgentMessageThreadItem):
        return thread_item
    return None


def _final_assistant_response_from_items(items: list[ThreadItem]) -> str:
    last_unknown_phase_response: str | None = None

    for item in reversed(items):
        agent_message = _agent_message_item_from_thread_item(item)
        if agent_message is None:
            continue
        if agent_message.phase == MessagePhase.final_answer:
            return agent_message.text
        if agent_message.phase is None and last_unknown_phase_response is None:
            last_unknown_phase_response = agent_message.text

    return last_unknown_phase_response or ""


def _raise_for_failed_turn(turn: AppServerTurn) -> None:
    if turn.status != TurnStatus.failed:
        return
    if turn.error is not None and turn.error.message:
        raise RuntimeError(turn.error.message)
    raise RuntimeError(f"turn failed with status {turn.status.value}")


def _collect_run_result(stream: Iterator[Notification], *, turn_id: str) -> RunResult:
    completed: TurnCompletedNotification | None = None
    items: list[ThreadItem] = []
    usage: ThreadTokenUsage | None = None

    for event in stream:
        payload = event.payload
        if isinstance(payload, ItemCompletedNotification) and payload.turn_id == turn_id:
            items.append(payload.item)
            continue
        if isinstance(payload, ThreadTokenUsageUpdatedNotification) and payload.turn_id == turn_id:
            usage = payload.token_usage
            continue
        if isinstance(payload, TurnCompletedNotification) and payload.turn.id == turn_id:
            completed = payload

    if completed is None:
        raise RuntimeError("turn completed event not received")

    _raise_for_failed_turn(completed.turn)
    return RunResult(
        final_response=_final_assistant_response_from_items(items),
        items=items,
        usage=usage,
    )


async def _collect_async_run_result(
    stream: AsyncIterator[Notification], *, turn_id: str
) -> RunResult:
    completed: TurnCompletedNotification | None = None
    items: list[ThreadItem] = []
    usage: ThreadTokenUsage | None = None

    async for event in stream:
        payload = event.payload
        if isinstance(payload, ItemCompletedNotification) and payload.turn_id == turn_id:
            items.append(payload.item)
            continue
        if isinstance(payload, ThreadTokenUsageUpdatedNotification) and payload.turn_id == turn_id:
            usage = payload.token_usage
            continue
        if isinstance(payload, TurnCompletedNotification) and payload.turn.id == turn_id:
            completed = payload

    if completed is None:
        raise RuntimeError("turn completed event not received")

    _raise_for_failed_turn(completed.turn)
    return RunResult(
        final_response=_final_assistant_response_from_items(items),
        items=items,
        usage=usage,
    )
