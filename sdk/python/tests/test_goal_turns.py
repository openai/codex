import asyncio
import threading
from dataclasses import dataclass

import pytest

from openai_codex.api import AsyncTurnHandle, TurnHandle
from openai_codex.client import CodexClient
from openai_codex.errors import InvalidRequestError
from openai_codex.generated.notification_registry import notification_turn_id
from openai_codex.generated.v2_all import (
    TurnCompletedNotification,
    TurnInterruptResponse,
    TurnStatus,
    TurnSteerResponse,
)


def _route(client: CodexClient, method: str, params: dict[str, object]) -> None:
    client._router.route_notification(client._coerce_notification(method, params))


def _turn(turn_id: str, status: str, *, started_at: int, completed_at: int | None = None):
    turn: dict[str, object] = {
        "id": turn_id,
        "items": [],
        "startedAt": started_at,
        "status": status,
    }
    if completed_at is not None:
        turn["completedAt"] = completed_at
    return turn


def _goal(thread_id: str, status: str) -> dict[str, object]:
    return {
        "createdAt": 1,
        "objective": "Improve benchmark coverage",
        "status": status,
        "threadId": thread_id,
        "timeUsedSeconds": 0,
        "tokensUsed": 0,
        "updatedAt": 1,
    }


def test_goal_turn_collects_continuations_as_one_logical_turn() -> None:
    client = CodexClient()
    state = client.register_goal_operation("thread-1")
    state.bind("turn-1")
    handle = TurnHandle(client, "thread-1", "turn-1", _goal=state)

    _route(
        client,
        "thread/goal/updated",
        {"threadId": "thread-1", "turnId": "turn-1", "goal": _goal("thread-1", "active")},
    )
    _route(
        client,
        "turn/started",
        {"threadId": "thread-1", "turn": _turn("turn-1", "inProgress", started_at=10)},
    )
    _route(
        client,
        "item/completed",
        {
            "threadId": "thread-1",
            "turnId": "turn-1",
            "item": {"id": "message-1", "type": "agentMessage", "text": "working"},
        },
    )
    _route(
        client,
        "turn/completed",
        {
            "threadId": "thread-1",
            "turn": _turn("turn-1", "completed", started_at=10, completed_at=12),
        },
    )
    _route(
        client,
        "turn/started",
        {"threadId": "thread-1", "turn": _turn("turn-2", "inProgress", started_at=13)},
    )
    _route(
        client,
        "item/completed",
        {
            "threadId": "thread-1",
            "turnId": "turn-2",
            "item": {
                "id": "message-2",
                "type": "agentMessage",
                "text": "done",
                "phase": "final_answer",
            },
        },
    )
    _route(
        client,
        "thread/goal/updated",
        {"threadId": "thread-1", "turnId": "turn-2", "goal": _goal("thread-1", "complete")},
    )
    _route(
        client,
        "thread/tokenUsage/updated",
        {
            "threadId": "thread-1",
            "turnId": "turn-2",
            "tokenUsage": {
                "last": {
                    "cachedInputTokens": 1,
                    "inputTokens": 2,
                    "outputTokens": 3,
                    "reasoningOutputTokens": 4,
                    "totalTokens": 9,
                },
                "total": {
                    "cachedInputTokens": 10,
                    "inputTokens": 20,
                    "outputTokens": 30,
                    "reasoningOutputTokens": 40,
                    "totalTokens": 90,
                },
            },
        },
    )
    _route(
        client,
        "turn/completed",
        {
            "threadId": "thread-1",
            "turn": _turn("turn-2", "completed", started_at=13, completed_at=15),
        },
    )

    result = handle.run()

    assert {
        "id": result.id,
        "status": result.status,
        "started_at": result.started_at,
        "completed_at": result.completed_at,
        "duration_ms": result.duration_ms,
        "final_response": result.final_response,
        "item_count": len(result.items),
        "total_tokens": result.usage.total.total_tokens if result.usage is not None else None,
        "registered_goals": client._router._goal_operations,
    } == {
        "id": "turn-1",
        "status": TurnStatus.completed,
        "started_at": 10,
        "completed_at": 15,
        "duration_ms": 5000,
        "final_response": "done",
        "item_count": 2,
        "total_tokens": 90,
        "registered_goals": {},
    }


def test_goal_stream_hides_continuation_boundaries_and_rewrites_ids() -> None:
    client = CodexClient()
    state = client.register_goal_operation("thread-1")
    state.bind("turn-1")
    handle = TurnHandle(client, "thread-1", "turn-1", _goal=state)

    for method, params in [
        (
            "thread/goal/updated",
            {"threadId": "thread-1", "turnId": "turn-1", "goal": _goal("thread-1", "active")},
        ),
        (
            "turn/started",
            {"threadId": "thread-1", "turn": _turn("turn-1", "inProgress", started_at=10)},
        ),
        (
            "turn/completed",
            {
                "threadId": "thread-1",
                "turn": _turn("turn-1", "completed", started_at=10, completed_at=11),
            },
        ),
        (
            "turn/started",
            {"threadId": "thread-1", "turn": _turn("turn-2", "inProgress", started_at=12)},
        ),
        (
            "thread/goal/updated",
            {"threadId": "thread-1", "turnId": "turn-2", "goal": _goal("thread-1", "complete")},
        ),
        (
            "turn/completed",
            {
                "threadId": "thread-1",
                "turn": _turn("turn-2", "completed", started_at=12, completed_at=13),
            },
        ),
    ]:
        _route(client, method, params)

    events = list(handle.stream())
    lifecycle = [event for event in events if event.method in {"turn/started", "turn/completed"}]

    assert {
        "lifecycle": [event.method for event in lifecycle],
        "turn_ids": [
            turn_id
            for event in events
            if (turn_id := notification_turn_id(event.payload)) is not None
        ],
        "completed": [
            event.payload.turn.status
            for event in events
            if isinstance(event.payload, TurnCompletedNotification)
        ],
    } == {
        "lifecycle": ["turn/started", "turn/completed"],
        "turn_ids": ["turn-1", "turn-1"],
        "completed": [TurnStatus.completed],
    }


def test_goal_router_releases_and_wakes_operations_on_transport_failure() -> None:
    client = CodexClient()
    state = client.register_goal_operation("thread-1")
    failure = RuntimeError("transport closed")

    client._router.fail_all(failure)

    with pytest.raises(RuntimeError, match="transport closed"):
        state.next_notification()
    assert client._router._goal_operations == {}


def test_closing_unstarted_goal_stream_releases_route_and_controls() -> None:
    client = CodexClient()
    state = client.register_goal_operation("thread-1")
    state.bind("turn-1")
    handle = TurnHandle(client, "thread-1", "turn-1", _goal=state)

    stream = handle.stream()
    stream.close()

    assert {
        "active_turn": state.active_turn(),
        "registered_goals": client._router._goal_operations,
    } == {"active_turn": None, "registered_goals": {}}


def test_failed_goal_start_propagates_turn_error() -> None:
    client = CodexClient()
    state = client.register_goal_operation("thread-1")
    state.bind("turn-1")
    handle = TurnHandle(client, "thread-1", "turn-1", _goal=state)
    failed_turn = _turn("turn-1", "failed", started_at=10, completed_at=11)
    failed_turn["error"] = {"message": "failed to start goal"}

    _route(
        client,
        "turn/started",
        {"threadId": "thread-1", "turn": _turn("turn-1", "inProgress", started_at=10)},
    )
    _route(
        client,
        "turn/completed",
        {"threadId": "thread-1", "turn": failed_turn},
    )

    with pytest.raises(RuntimeError, match="failed to start goal"):
        handle.run()
    assert client._router._goal_operations == {}


class _SyncControlClient:
    def __init__(self, *, fail_first: bool = True) -> None:
        self.steer_ids: list[str] = []
        self.interrupt_ids: list[str] = []
        self.paused = False
        self.fail_first = fail_first

    def turn_steer(self, _thread_id, turn_id, _input):
        self.steer_ids.append(turn_id)
        if self.fail_first and len(self.steer_ids) == 1:
            raise InvalidRequestError(
                -32600,
                "expected active turn id `turn-1` but found `turn-2`",
            )
        return TurnSteerResponse(turn_id=turn_id)

    def pause_goal(self, _thread_id):
        self.paused = True

    def turn_interrupt(self, _thread_id, turn_id):
        self.interrupt_ids.append(turn_id)
        if self.fail_first and len(self.interrupt_ids) == 1:
            raise InvalidRequestError(
                -32600,
                "expected active turn id turn-1 but found turn-2",
            )
        return TurnInterruptResponse()


def test_goal_controls_hide_rollover_turn_ids() -> None:
    state = CodexClient().register_goal_operation("thread-1")
    state.bind("turn-1")
    client = _SyncControlClient()
    handle = TurnHandle(client, "thread-1", "turn-1", _goal=state)  # type: ignore[arg-type]

    steered = handle.steer("keep going")
    interrupted = handle.interrupt()

    assert {
        "steer_ids": client.steer_ids,
        "public_steer_id": steered.turn_id,
        "paused": client.paused,
        "interrupt_ids": client.interrupt_ids,
        "interrupt": interrupted.model_dump(mode="json"),
    } == {
        "steer_ids": ["turn-1", "turn-2"],
        "public_steer_id": "turn-1",
        "paused": True,
        "interrupt_ids": ["turn-1", "turn-2"],
        "interrupt": {},
    }


def test_finished_goal_controls_match_inactive_turn_errors() -> None:
    state = CodexClient().register_goal_operation("thread-1")
    state.bind("turn-1")
    state.finish()
    client = _SyncControlClient(fail_first=False)
    handle = TurnHandle(client, "thread-1", "turn-1", _goal=state)  # type: ignore[arg-type]

    with pytest.raises(InvalidRequestError, match="no active turn to steer"):
        handle.steer("keep going")
    with pytest.raises(InvalidRequestError, match="no active turn to interrupt"):
        handle.interrupt()
    assert client.paused is False


def test_goal_steer_waits_for_rollover_turn() -> None:
    client = CodexClient()
    state = client.register_goal_operation("thread-1")
    state.bind("turn-1")
    control = _SyncControlClient(fail_first=False)
    handle = TurnHandle(control, "thread-1", "turn-1", _goal=state)  # type: ignore[arg-type]
    results: list[TurnSteerResponse] = []

    _route(
        client,
        "thread/goal/updated",
        {"threadId": "thread-1", "turnId": "turn-1", "goal": _goal("thread-1", "active")},
    )
    _route(
        client,
        "turn/started",
        {"threadId": "thread-1", "turn": _turn("turn-1", "inProgress", started_at=10)},
    )
    _route(
        client,
        "turn/completed",
        {
            "threadId": "thread-1",
            "turn": _turn("turn-1", "completed", started_at=10, completed_at=11),
        },
    )

    steering = threading.Thread(target=lambda: results.append(handle.steer("continue")))
    steering.start()
    _route(
        client,
        "turn/started",
        {"threadId": "thread-1", "turn": _turn("turn-2", "inProgress", started_at=12)},
    )
    steering.join(timeout=5)

    assert {
        "alive": steering.is_alive(),
        "steer_ids": control.steer_ids,
        "public_turn_id": results[0].turn_id,
    } == {
        "alive": False,
        "steer_ids": ["turn-2"],
        "public_turn_id": "turn-1",
    }


def test_goal_interrupt_succeeds_during_rollover() -> None:
    client = CodexClient()
    state = client.register_goal_operation("thread-1")
    state.bind("turn-1")
    control = _SyncControlClient()
    handle = TurnHandle(control, "thread-1", "turn-1", _goal=state)  # type: ignore[arg-type]
    _route(
        client,
        "thread/goal/updated",
        {"threadId": "thread-1", "turnId": "turn-1", "goal": _goal("thread-1", "active")},
    )
    _route(
        client,
        "turn/started",
        {"threadId": "thread-1", "turn": _turn("turn-1", "inProgress", started_at=10)},
    )
    _route(
        client,
        "turn/completed",
        {
            "threadId": "thread-1",
            "turn": _turn("turn-1", "completed", started_at=10, completed_at=11),
        },
    )

    response = handle.interrupt()

    assert {
        "paused": control.paused,
        "interrupt_ids": control.interrupt_ids,
        "response": response.model_dump(mode="json"),
        "interrupted": state.interrupted,
    } == {
        "paused": True,
        "interrupt_ids": [],
        "response": {},
        "interrupted": True,
    }


class _AsyncGoalClient:
    def __init__(self, state) -> None:
        self.state = state
        self.unregistered = False

    async def next_goal_notification(self, state):
        return await asyncio.to_thread(state.next_notification)

    def unregister_goal_operation(self, _state) -> None:
        self.unregistered = True


@dataclass(slots=True)
class _AsyncCodexStub:
    _client: object

    async def _ensure_initialized(self) -> None:
        return None


def test_async_goal_stream_matches_sync_logical_lifecycle() -> None:
    async def scenario() -> None:
        client = CodexClient()
        state = client.register_goal_operation("thread-1")
        state.bind("turn-1")
        async_client = _AsyncGoalClient(state)
        codex = _AsyncCodexStub(async_client)
        handle = AsyncTurnHandle(codex, "thread-1", "turn-1", _goal=state)  # type: ignore[arg-type]

        for method, params in [
            (
                "thread/goal/updated",
                {
                    "threadId": "thread-1",
                    "turnId": "turn-1",
                    "goal": _goal("thread-1", "active"),
                },
            ),
            (
                "turn/started",
                {"threadId": "thread-1", "turn": _turn("turn-1", "inProgress", started_at=10)},
            ),
            (
                "thread/goal/updated",
                {
                    "threadId": "thread-1",
                    "turnId": "turn-1",
                    "goal": _goal("thread-1", "complete"),
                },
            ),
            (
                "turn/completed",
                {
                    "threadId": "thread-1",
                    "turn": _turn("turn-1", "completed", started_at=10, completed_at=11),
                },
            ),
        ]:
            _route(client, method, params)

        events = [event async for event in handle.stream()]

        assert {
            "methods": [event.method for event in events],
            "ids": [notification_turn_id(event.payload) for event in events],
            "unregistered": async_client.unregistered,
        } == {
            "methods": ["turn/started", "turn/completed"],
            "ids": ["turn-1", "turn-1"],
            "unregistered": True,
        }

    asyncio.run(scenario())


def test_cancelling_async_goal_stream_wakes_notification_reader() -> None:
    async def scenario() -> None:
        state = CodexClient().register_goal_operation("thread-1")
        state.bind("turn-1")
        async_client = _AsyncGoalClient(state)
        codex = _AsyncCodexStub(async_client)
        handle = AsyncTurnHandle(codex, "thread-1", "turn-1", _goal=state)  # type: ignore[arg-type]
        stream = handle.stream()
        reading = asyncio.create_task(anext(stream))
        await asyncio.sleep(0)

        reading.cancel()
        with pytest.raises(asyncio.CancelledError):
            await reading

        assert {"finished": state.is_finished(), "unregistered": async_client.unregistered} == {
            "finished": True,
            "unregistered": True,
        }

    asyncio.run(scenario())


class _AsyncControlClient:
    def __init__(self) -> None:
        self.steer_ids: list[str] = []
        self.interrupt_ids: list[str] = []
        self.paused = False

    async def turn_steer(self, _thread_id, turn_id, _input):
        self.steer_ids.append(turn_id)
        if len(self.steer_ids) == 1:
            raise InvalidRequestError(
                -32600,
                "expected active turn id `turn-1` but found `turn-2`",
            )
        return TurnSteerResponse(turn_id=turn_id)

    async def pause_goal(self, _thread_id):
        self.paused = True

    async def turn_interrupt(self, _thread_id, turn_id):
        self.interrupt_ids.append(turn_id)
        if len(self.interrupt_ids) == 1:
            raise InvalidRequestError(
                -32600,
                "expected active turn id turn-1 but found turn-2",
            )
        return TurnInterruptResponse()


def test_async_goal_controls_hide_rollover_turn_ids() -> None:
    async def scenario() -> None:
        state = CodexClient().register_goal_operation("thread-1")
        state.bind("turn-1")
        client = _AsyncControlClient()
        codex = _AsyncCodexStub(client)  # type: ignore[arg-type]
        handle = AsyncTurnHandle(codex, "thread-1", "turn-1", _goal=state)  # type: ignore[arg-type]

        steered = await handle.steer("keep going")
        interrupted = await handle.interrupt()

        assert {
            "steer_ids": client.steer_ids,
            "public_steer_id": steered.turn_id,
            "paused": client.paused,
            "interrupt_ids": client.interrupt_ids,
            "interrupt": interrupted.model_dump(mode="json"),
        } == {
            "steer_ids": ["turn-1", "turn-2"],
            "public_steer_id": "turn-1",
            "paused": True,
            "interrupt_ids": ["turn-1", "turn-2"],
            "interrupt": {},
        }

    asyncio.run(scenario())


def test_finished_async_goal_controls_match_inactive_turn_errors() -> None:
    async def scenario() -> None:
        state = CodexClient().register_goal_operation("thread-1")
        state.bind("turn-1")
        state.finish()
        client = _AsyncControlClient()
        codex = _AsyncCodexStub(client)
        handle = AsyncTurnHandle(codex, "thread-1", "turn-1", _goal=state)  # type: ignore[arg-type]

        with pytest.raises(InvalidRequestError, match="no active turn to steer"):
            await handle.steer("keep going")
        with pytest.raises(InvalidRequestError, match="no active turn to interrupt"):
            await handle.interrupt()
        assert client.paused is False

    asyncio.run(scenario())
