from __future__ import annotations

import asyncio
from collections import deque
from pathlib import Path
from types import SimpleNamespace
from typing import Any

import pytest

import openai_codex.api as public_api_module
from openai_codex.client import AppServerClient
from openai_codex.generated.v2_all import (
    AgentMessageDeltaNotification,
    ItemCompletedNotification,
    MessagePhase,
    TurnCompletedNotification,
    TurnStartParams,
)
from openai_codex.models import InitializeResponse, Notification
from openai_codex.api import (
    ApprovalMode,
    AsyncCodex,
    AsyncThread,
    Codex,
    Thread,
)

ROOT = Path(__file__).resolve().parents[1]


def _approval_settings(params: list[Any]) -> list[dict[str, object]]:
    """Return serialized approval settings from captured Pydantic params."""
    return [
        {
            key: value
            for key, value in param.model_dump(
                by_alias=True,
                exclude_none=True,
                mode="json",
            ).items()
            if key in {"approvalPolicy", "approvalsReviewer"}
        }
        for param in params
    ]


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
    phase: MessagePhase | None = None,
) -> Notification:
    """Build a realistic completed-item notification accepted by generated models."""
    item: dict[str, object] = {
        "id": "item-1",
        "text": text,
        "type": "agentMessage",
    }
    if phase is not None:
        item["phase"] = phase.value
    return Notification(
        method="item/completed",
        payload=ItemCompletedNotification.model_validate(
            {
                # The pinned runtime schema requires completion timestamps.
                "completedAtMs": 1,
                "item": item,
                "threadId": thread_id,
                "turnId": turn_id,
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


def _approval_mode_turn_params(approval_mode: ApprovalMode) -> TurnStartParams:
    """Build real generated turn params from one public approval mode."""
    approval_policy, approvals_reviewer = public_api_module._approval_mode_settings(
        approval_mode
    )
    return TurnStartParams(
        thread_id="thread-1",
        input=[],
        approval_policy=approval_policy,
        approvals_reviewer=approvals_reviewer,
    )


def test_approval_modes_serialize_to_expected_start_params() -> None:
    """ApprovalMode should map to the app-server params sent for new work."""
    assert {
        mode.value: _approval_settings([_approval_mode_turn_params(mode)])[0]
        for mode in ApprovalMode
    } == {
        "deny_all": {"approvalPolicy": "never"},
        "auto_review": {
            "approvalPolicy": "on-request",
            "approvalsReviewer": "auto_review",
        },
    }


def test_unknown_approval_mode_is_rejected() -> None:
    """Invalid approval modes should fail before params are constructed."""
    with pytest.raises(ValueError, match="deny_all, auto_review"):
        public_api_module._approval_mode_settings("allow_all")  # type: ignore[arg-type]


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
    client.next_turn_notification = lambda _turn_id: notifications.popleft()  # type: ignore[method-assign]
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
    client.next_turn_notification = lambda _turn_id: notifications.popleft()  # type: ignore[method-assign]
    client.turn_start = lambda thread_id, wire_input, *, params=None: SimpleNamespace(  # noqa: ARG005,E731
        turn=SimpleNamespace(id="turn-1")
    )

    result = Thread(client, "thread-1").run("hello")

    assert result.final_response == ""
    assert result.items == [
        first_item_notification.payload.item,
        second_item_notification.payload.item,
    ]


def test_thread_run_prefers_explicit_final_answer_over_later_commentary() -> None:
    client = AppServerClient()
    final_answer_notification = _item_completed_notification(
        text="Final answer",
        phase=MessagePhase.final_answer,
    )
    commentary_notification = _item_completed_notification(
        text="Commentary",
        phase=MessagePhase.commentary,
    )
    notifications: deque[Notification] = deque(
        [
            final_answer_notification,
            commentary_notification,
            _completed_notification(),
        ]
    )
    client.next_turn_notification = lambda _turn_id: notifications.popleft()  # type: ignore[method-assign]
    client.turn_start = lambda thread_id, wire_input, *, params=None: SimpleNamespace(  # noqa: ARG005,E731
        turn=SimpleNamespace(id="turn-1")
    )

    result = Thread(client, "thread-1").run("hello")

    assert result.final_response == "Final answer"
    assert result.items == [
        final_answer_notification.payload.item,
        commentary_notification.payload.item,
    ]


def test_thread_run_returns_none_when_only_commentary_messages_complete() -> None:
    client = AppServerClient()
    commentary_notification = _item_completed_notification(
        text="Commentary",
        phase=MessagePhase.commentary,
    )
    notifications: deque[Notification] = deque(
        [
            commentary_notification,
            _completed_notification(),
        ]
    )
    client.next_turn_notification = lambda _turn_id: notifications.popleft()  # type: ignore[method-assign]
    client.turn_start = lambda thread_id, wire_input, *, params=None: SimpleNamespace(  # noqa: ARG005,E731
        turn=SimpleNamespace(id="turn-1")
    )

    result = Thread(client, "thread-1").run("hello")

    assert result.final_response is None
    assert result.items == [commentary_notification.payload.item]


def test_thread_run_raises_on_failed_turn() -> None:
    client = AppServerClient()
    notifications: deque[Notification] = deque(
        [
            _completed_notification(status="failed", error_message="boom"),
        ]
    )
    client.next_turn_notification = lambda _turn_id: notifications.popleft()  # type: ignore[method-assign]
    client.turn_start = lambda thread_id, wire_input, *, params=None: SimpleNamespace(  # noqa: ARG005,E731
        turn=SimpleNamespace(id="turn-1")
    )

    with pytest.raises(RuntimeError, match="boom"):
        Thread(client, "thread-1").run("hello")


def test_stream_text_registers_and_consumes_turn_notifications() -> None:
    client = AppServerClient()
    notifications: deque[Notification] = deque(
        [
            _delta_notification(text="first"),
            _delta_notification(text="second"),
            _completed_notification(),
        ]
    )
    calls: list[tuple[str, str]] = []
    client.turn_start = lambda thread_id, input_items, *, params=None: SimpleNamespace(  # noqa: ARG005,E731
        turn=SimpleNamespace(id="turn-1")
    )

    def fake_register(turn_id: str) -> None:
        calls.append(("register", turn_id))

    def fake_next(turn_id: str) -> Notification:
        calls.append(("next", turn_id))
        return notifications.popleft()

    def fake_unregister(turn_id: str) -> None:
        calls.append(("unregister", turn_id))

    client.register_turn_notifications = fake_register  # type: ignore[method-assign]
    client.next_turn_notification = fake_next  # type: ignore[method-assign]
    client.unregister_turn_notifications = fake_unregister  # type: ignore[method-assign]

    chunks = list(client.stream_text("thread-1", "hello"))

    assert ([chunk.delta for chunk in chunks], calls) == (
        ["first", "second"],
        [
            ("register", "turn-1"),
            ("next", "turn-1"),
            ("next", "turn-1"),
            ("next", "turn-1"),
            ("unregister", "turn-1"),
        ],
    )


def test_async_thread_run_uses_last_completed_assistant_message_as_final_response() -> (
    None
):
    """Async run should use the last final assistant message as the response text."""

    async def scenario() -> None:
        codex = AsyncCodex()

        async def fake_ensure_initialized() -> None:
            return None

        first_item_notification = _item_completed_notification(
            text="First async message"
        )
        second_item_notification = _item_completed_notification(
            text="Second async message"
        )
        notifications: deque[Notification] = deque(
            [
                first_item_notification,
                second_item_notification,
                _completed_notification(),
            ]
        )

        async def fake_turn_start(thread_id: str, wire_input: object, *, params=None):  # noqa: ANN001,ANN202,ARG001
            return SimpleNamespace(turn=SimpleNamespace(id="turn-1"))

        async def fake_next_notification(_turn_id: str) -> Notification:
            return notifications.popleft()

        codex._ensure_initialized = fake_ensure_initialized  # type: ignore[method-assign]
        codex._client.turn_start = fake_turn_start  # type: ignore[method-assign]
        codex._client.next_turn_notification = fake_next_notification  # type: ignore[method-assign]

        result = await AsyncThread(codex, "thread-1").run("hello")

        assert result.final_response == "Second async message"
        assert result.items == [
            first_item_notification.payload.item,
            second_item_notification.payload.item,
        ]

    asyncio.run(scenario())


def test_async_thread_run_returns_none_when_only_commentary_messages_complete() -> None:
    """Async Thread.run should ignore commentary-only messages for final text."""

    async def scenario() -> None:
        codex = AsyncCodex()

        async def fake_ensure_initialized() -> None:
            return None

        commentary_notification = _item_completed_notification(
            text="Commentary",
            phase=MessagePhase.commentary,
        )
        notifications: deque[Notification] = deque(
            [
                commentary_notification,
                _completed_notification(),
            ]
        )

        async def fake_turn_start(thread_id: str, wire_input: object, *, params=None):  # noqa: ANN001,ANN202,ARG001
            return SimpleNamespace(turn=SimpleNamespace(id="turn-1"))

        async def fake_next_notification(_turn_id: str) -> Notification:
            return notifications.popleft()

        codex._ensure_initialized = fake_ensure_initialized  # type: ignore[method-assign]
        codex._client.turn_start = fake_turn_start  # type: ignore[method-assign]
        codex._client.next_turn_notification = fake_next_notification  # type: ignore[method-assign]

        result = await AsyncThread(codex, "thread-1").run("hello")

        assert result.final_response is None
        assert result.items == [commentary_notification.payload.item]

    asyncio.run(scenario())


def test_retry_examples_compare_status_with_enum() -> None:
    for path in (
        ROOT / "examples" / "10_error_handling_and_retry" / "sync.py",
        ROOT / "examples" / "10_error_handling_and_retry" / "async.py",
    ):
        source = path.read_text()
        assert '== "failed"' not in source
        assert "TurnStatus.failed" in source
