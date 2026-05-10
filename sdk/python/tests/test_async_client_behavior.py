from __future__ import annotations

import asyncio
import time

from openai_codex.async_client import AsyncAppServerClient
from openai_codex.generated.v2_all import (
    TurnCompletedNotification,
)
from openai_codex.models import Notification, UnknownNotification


def test_async_client_allows_concurrent_transport_calls() -> None:
    """Async wrappers should offload sync calls so concurrent awaits can overlap."""
    async def scenario() -> int:
        """Run two blocking sync calls and report peak overlap."""
        client = AsyncAppServerClient()
        active = 0
        max_active = 0

        def fake_model_list(include_hidden: bool = False) -> bool:
            """Simulate a blocking sync transport call."""
            nonlocal active, max_active
            active += 1
            max_active = max(max_active, active)
            time.sleep(0.05)
            active -= 1
            return include_hidden

        client._sync.model_list = fake_model_list  # type: ignore[method-assign]
        await asyncio.gather(client.model_list(), client.model_list())
        return max_active

    assert asyncio.run(scenario()) == 2


def test_async_client_turn_notification_methods_delegate_to_sync_client() -> None:
    """Async turn routing methods should preserve sync-client registration semantics."""
    async def scenario() -> tuple[list[tuple[str, str]], Notification, str]:
        """Record the sync-client calls made by async turn notification wrappers."""
        client = AsyncAppServerClient()
        event = Notification(
            method="unknown/direct",
            payload=UnknownNotification(params={"turnId": "turn-1"}),
        )
        completed = TurnCompletedNotification.model_validate(
            {
                "threadId": "thread-1",
                "turn": {"id": "turn-1", "items": [], "status": "completed"},
            }
        )
        calls: list[tuple[str, str]] = []

        def fake_register(turn_id: str) -> None:
            """Record turn registration through the wrapped sync client."""
            calls.append(("register", turn_id))

        def fake_unregister(turn_id: str) -> None:
            """Record turn unregistration through the wrapped sync client."""
            calls.append(("unregister", turn_id))

        def fake_next(turn_id: str) -> Notification:
            """Return one routed notification through the wrapped sync client."""
            calls.append(("next", turn_id))
            return event

        def fake_wait(turn_id: str) -> TurnCompletedNotification:
            """Return one completion through the wrapped sync client."""
            calls.append(("wait", turn_id))
            return completed

        client._sync.register_turn_notifications = fake_register  # type: ignore[method-assign]
        client._sync.unregister_turn_notifications = fake_unregister  # type: ignore[method-assign]
        client._sync.next_turn_notification = fake_next  # type: ignore[method-assign]
        client._sync.wait_for_turn_completed = fake_wait  # type: ignore[method-assign]

        client.register_turn_notifications("turn-1")
        next_event = await client.next_turn_notification("turn-1")
        completed_event = await client.wait_for_turn_completed("turn-1")
        client.unregister_turn_notifications("turn-1")

        return calls, next_event, completed_event.turn.id

    calls, next_event, completed_turn_id = asyncio.run(scenario())

    assert (
        calls,
        next_event,
        completed_turn_id,
    ) == (
        [
            ("register", "turn-1"),
            ("next", "turn-1"),
            ("wait", "turn-1"),
            ("unregister", "turn-1"),
        ],
        Notification(
            method="unknown/direct",
            payload=UnknownNotification(params={"turnId": "turn-1"}),
        ),
        "turn-1",
    )
