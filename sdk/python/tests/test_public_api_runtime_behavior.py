from __future__ import annotations

import asyncio
from collections import deque
from pathlib import Path
from types import SimpleNamespace

import pytest

import codex_app_server.api as public_api_module
from codex_app_server.client import AppServerClient
from codex_app_server.generated.v2_all import (
    AgentMessageDeltaNotification,
    ItemCompletedNotification,
    ThreadTokenUsageUpdatedNotification,
    TurnCompletedNotification,
    TurnStatus,
)
from codex_app_server.models import InitializeResponse, Notification
from codex_app_server.api import (
    AsyncCodex,
    AsyncThread,
    AsyncTurnHandle,
    Codex,
    RunResult,
    Thread,
    TurnHandle,
)

ROOT = Path(__file__).resolve().parents[1]


def _delta_notification(
    *,
    thread_id: str = "thread-1",
    turn_id: str = "turn-1",
    text: str = "delta-text",
) -> Notification:
    return Notification(
        method="item/agentMessage/delta",
        payload=AgentMessageDeltaNotification.model_validate(
            {
                "delta": text,
                "itemId": "item-1",
                "threadId": thread_id,
                "turnId": turn_id,
            }
        ),
    )


def _completed_notification(
    *,
    thread_id: str = "thread-1",
    turn_id: str = "turn-1",
    status: str = "completed",
    error_message: str | None = None,
) -> Notification:
    turn: dict[str, object] = {
        "id": turn_id,
        "items": [],
        "status": status,
    }
    if error_message is not None:
        turn["error"] = {"message": error_message}
    return Notification(
        method="turn/completed",
        payload=TurnCompletedNotification.model_validate(
            {
                "threadId": thread_id,
                "turn": turn,
            }
        ),
    )


def _item_completed_notification(
    *,
    thread_id: str = "thread-1",
    turn_id: str = "turn-1",
    text: str = "final text",
) -> Notification:
    return Notification(
        method="item/completed",
        payload=ItemCompletedNotification.model_validate(
            {
                "item": {
                    "id": "item-1",
                    "text": text,
                    "type": "agentMessage",
                },
                "threadId": thread_id,
                "turnId": turn_id,
            }
        ),
    )


def _token_usage_notification(
    *,
    thread_id: str = "thread-1",
    turn_id: str = "turn-1",
) -> Notification:
    return Notification(
        method="thread/tokenUsage/updated",
        payload=ThreadTokenUsageUpdatedNotification.model_validate(
            {
                "threadId": thread_id,
                "turnId": turn_id,
                "tokenUsage": {
                    "last": {
                        "cachedInputTokens": 1,
                        "inputTokens": 2,
                        "outputTokens": 3,
                        "reasoningOutputTokens": 4,
                        "totalTokens": 9,
                    },
                    "total": {
                        "cachedInputTokens": 5,
                        "inputTokens": 6,
                        "outputTokens": 7,
                        "reasoningOutputTokens": 8,
                        "totalTokens": 26,
                    },
                },
            }
        ),
    )


def test_codex_init_failure_closes_client(monkeypatch: pytest.MonkeyPatch) -> None:
    closed: list[bool] = []

    class FakeClient:
        def __init__(self, config=None) -> None:  # noqa: ANN001,ARG002
            self._closed = False

        def start(self) -> None:
            return None

        def initialize(self) -> InitializeResponse:
            return InitializeResponse.model_validate({})

        def close(self) -> None:
            self._closed = True
            closed.append(True)

    monkeypatch.setattr(public_api_module, "AppServerClient", FakeClient)

    with pytest.raises(RuntimeError, match="missing required metadata"):
        Codex()

    assert closed == [True]


def test_async_codex_init_failure_closes_client() -> None:
    async def scenario() -> None:
        codex = AsyncCodex()
        close_calls = 0

        async def fake_start() -> None:
            return None

        async def fake_initialize() -> InitializeResponse:
            return InitializeResponse.model_validate({})

        async def fake_close() -> None:
            nonlocal close_calls
            close_calls += 1

        codex._client.start = fake_start  # type: ignore[method-assign]
        codex._client.initialize = fake_initialize  # type: ignore[method-assign]
        codex._client.close = fake_close  # type: ignore[method-assign]

        with pytest.raises(RuntimeError, match="missing required metadata"):
            await codex.models()

        assert close_calls == 1
        assert codex._initialized is False
        assert codex._init is None

    asyncio.run(scenario())


def test_async_codex_initializes_only_once_under_concurrency() -> None:
    async def scenario() -> None:
        codex = AsyncCodex()
        start_calls = 0
        initialize_calls = 0
        ready = asyncio.Event()

        async def fake_start() -> None:
            nonlocal start_calls
            start_calls += 1

        async def fake_initialize() -> InitializeResponse:
            nonlocal initialize_calls
            initialize_calls += 1
            ready.set()
            await asyncio.sleep(0.02)
            return InitializeResponse.model_validate(
                {
                    "userAgent": "codex-cli/1.2.3",
                    "serverInfo": {"name": "codex-cli", "version": "1.2.3"},
                }
            )

        async def fake_model_list(include_hidden: bool = False):  # noqa: ANN202,ARG001
            await ready.wait()
            return object()

        codex._client.start = fake_start  # type: ignore[method-assign]
        codex._client.initialize = fake_initialize  # type: ignore[method-assign]
        codex._client.model_list = fake_model_list  # type: ignore[method-assign]

        await asyncio.gather(codex.models(), codex.models())

        assert start_calls == 1
        assert initialize_calls == 1

    asyncio.run(scenario())


def test_turn_stream_rejects_second_active_consumer() -> None:
    client = AppServerClient()
    notifications: deque[Notification] = deque(
        [
            _delta_notification(turn_id="turn-1"),
            _completed_notification(turn_id="turn-1"),
        ]
    )
    client.next_notification = notifications.popleft  # type: ignore[method-assign]

    first_stream = TurnHandle(client, "thread-1", "turn-1").stream()
    assert next(first_stream).method == "item/agentMessage/delta"

    second_stream = TurnHandle(client, "thread-1", "turn-2").stream()
    with pytest.raises(RuntimeError, match="Concurrent turn consumers are not yet supported"):
        next(second_stream)

    first_stream.close()


def test_async_turn_stream_rejects_second_active_consumer() -> None:
    async def scenario() -> None:
        codex = AsyncCodex()

        async def fake_ensure_initialized() -> None:
            return None

        notifications: deque[Notification] = deque(
            [
                _delta_notification(turn_id="turn-1"),
                _completed_notification(turn_id="turn-1"),
            ]
        )

        async def fake_next_notification() -> Notification:
            return notifications.popleft()

        codex._ensure_initialized = fake_ensure_initialized  # type: ignore[method-assign]
        codex._client.next_notification = fake_next_notification  # type: ignore[method-assign]

        first_stream = AsyncTurnHandle(codex, "thread-1", "turn-1").stream()
        assert (await anext(first_stream)).method == "item/agentMessage/delta"

        second_stream = AsyncTurnHandle(codex, "thread-1", "turn-2").stream()
        with pytest.raises(RuntimeError, match="Concurrent turn consumers are not yet supported"):
            await anext(second_stream)

        await first_stream.aclose()

    asyncio.run(scenario())


def test_turn_run_returns_completed_turn_payload() -> None:
    client = AppServerClient()
    notifications: deque[Notification] = deque(
        [
            _completed_notification(),
        ]
    )
    client.next_notification = notifications.popleft  # type: ignore[method-assign]

    result = TurnHandle(client, "thread-1", "turn-1").run()

    assert result.id == "turn-1"
    assert result.status == TurnStatus.completed
    assert result.items == []


def test_thread_run_accepts_string_input_and_returns_run_result() -> None:
    client = AppServerClient()
    item_notification = _item_completed_notification(text="Hello.")
    usage_notification = _token_usage_notification()
    notifications: deque[Notification] = deque(
        [
            item_notification,
            usage_notification,
            _completed_notification(),
        ]
    )
    client.next_notification = notifications.popleft  # type: ignore[method-assign]
    seen: dict[str, object] = {}

    def fake_turn_start(thread_id: str, wire_input: object, *, params=None):  # noqa: ANN001,ANN202
        seen["thread_id"] = thread_id
        seen["wire_input"] = wire_input
        seen["params"] = params
        return SimpleNamespace(turn=SimpleNamespace(id="turn-1"))

    client.turn_start = fake_turn_start  # type: ignore[method-assign]

    result = Thread(client, "thread-1").run("hello")

    assert seen["thread_id"] == "thread-1"
    assert seen["wire_input"] == [{"type": "text", "text": "hello"}]
    assert result == RunResult(
        final_response="Hello.",
        items=[item_notification.payload.item],
        usage=usage_notification.payload.token_usage,
    )


def test_thread_run_uses_last_completed_assistant_message_as_final_response() -> None:
    client = AppServerClient()
    first_item_notification = _item_completed_notification(text="First message")
    second_item_notification = _item_completed_notification(text="Second message")
    notifications: deque[Notification] = deque(
        [
            first_item_notification,
            second_item_notification,
            _completed_notification(),
        ]
    )
    client.next_notification = notifications.popleft  # type: ignore[method-assign]
    client.turn_start = lambda thread_id, wire_input, *, params=None: SimpleNamespace(  # noqa: ARG005,E731
        turn=SimpleNamespace(id="turn-1")
    )

    result = Thread(client, "thread-1").run("hello")

    assert result.final_response == "Second message"
    assert result.items == [
        first_item_notification.payload.item,
        second_item_notification.payload.item,
    ]


def test_thread_run_preserves_empty_last_assistant_message() -> None:
    client = AppServerClient()
    first_item_notification = _item_completed_notification(text="First message")
    second_item_notification = _item_completed_notification(text="")
    notifications: deque[Notification] = deque(
        [
            first_item_notification,
            second_item_notification,
            _completed_notification(),
        ]
    )
    client.next_notification = notifications.popleft  # type: ignore[method-assign]
    client.turn_start = lambda thread_id, wire_input, *, params=None: SimpleNamespace(  # noqa: ARG005,E731
        turn=SimpleNamespace(id="turn-1")
    )

    result = Thread(client, "thread-1").run("hello")

    assert result.final_response == ""
    assert result.items == [
        first_item_notification.payload.item,
        second_item_notification.payload.item,
    ]


def test_thread_run_raises_on_failed_turn() -> None:
    client = AppServerClient()
    notifications: deque[Notification] = deque(
        [
            _completed_notification(status="failed", error_message="boom"),
        ]
    )
    client.next_notification = notifications.popleft  # type: ignore[method-assign]
    client.turn_start = lambda thread_id, wire_input, *, params=None: SimpleNamespace(  # noqa: ARG005,E731
        turn=SimpleNamespace(id="turn-1")
    )

    with pytest.raises(RuntimeError, match="boom"):
        Thread(client, "thread-1").run("hello")


def test_async_thread_run_accepts_string_input_and_returns_run_result() -> None:
    async def scenario() -> None:
        codex = AsyncCodex()

        async def fake_ensure_initialized() -> None:
            return None

        item_notification = _item_completed_notification(text="Hello async.")
        usage_notification = _token_usage_notification()
        notifications: deque[Notification] = deque(
            [
                item_notification,
                usage_notification,
                _completed_notification(),
            ]
        )
        seen: dict[str, object] = {}

        async def fake_turn_start(thread_id: str, wire_input: object, *, params=None):  # noqa: ANN001,ANN202
            seen["thread_id"] = thread_id
            seen["wire_input"] = wire_input
            seen["params"] = params
            return SimpleNamespace(turn=SimpleNamespace(id="turn-1"))

        async def fake_next_notification() -> Notification:
            return notifications.popleft()

        codex._ensure_initialized = fake_ensure_initialized  # type: ignore[method-assign]
        codex._client.turn_start = fake_turn_start  # type: ignore[method-assign]
        codex._client.next_notification = fake_next_notification  # type: ignore[method-assign]

        result = await AsyncThread(codex, "thread-1").run("hello")

        assert seen["thread_id"] == "thread-1"
        assert seen["wire_input"] == [{"type": "text", "text": "hello"}]
        assert result == RunResult(
            final_response="Hello async.",
            items=[item_notification.payload.item],
            usage=usage_notification.payload.token_usage,
        )

    asyncio.run(scenario())


def test_async_thread_run_uses_last_completed_assistant_message_as_final_response() -> None:
    async def scenario() -> None:
        codex = AsyncCodex()

        async def fake_ensure_initialized() -> None:
            return None

        first_item_notification = _item_completed_notification(text="First async message")
        second_item_notification = _item_completed_notification(text="Second async message")
        notifications: deque[Notification] = deque(
            [
                first_item_notification,
                second_item_notification,
                _completed_notification(),
            ]
        )

        async def fake_turn_start(thread_id: str, wire_input: object, *, params=None):  # noqa: ANN001,ANN202,ARG001
            return SimpleNamespace(turn=SimpleNamespace(id="turn-1"))

        async def fake_next_notification() -> Notification:
            return notifications.popleft()

        codex._ensure_initialized = fake_ensure_initialized  # type: ignore[method-assign]
        codex._client.turn_start = fake_turn_start  # type: ignore[method-assign]
        codex._client.next_notification = fake_next_notification  # type: ignore[method-assign]

        result = await AsyncThread(codex, "thread-1").run("hello")

        assert result.final_response == "Second async message"
        assert result.items == [
            first_item_notification.payload.item,
            second_item_notification.payload.item,
        ]

    asyncio.run(scenario())


def test_retry_examples_compare_status_with_enum() -> None:
    for path in (
        ROOT / "examples" / "10_error_handling_and_retry" / "sync.py",
        ROOT / "examples" / "10_error_handling_and_retry" / "async.py",
    ):
        source = path.read_text()
        assert '== "failed"' not in source
        assert "TurnStatus.failed" in source
