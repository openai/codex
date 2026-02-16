from __future__ import annotations

from dataclasses import dataclass
from typing import Any, AsyncIterator, Iterator

from .models import Notification
from .schema_types import TurnStartResponse as SchemaTurnStartResponse

if False:  # pragma: no cover
    from .async_client import AsyncAppServerClient
    from .client import AppServerClient


@dataclass(slots=True)
class Conversation:
    """Fluent thread-scoped helper over :class:`AppServerClient`."""

    client: "AppServerClient"
    thread_id: str

    def turn_start(
        self,
        input_items: list[dict[str, Any]] | dict[str, Any] | str,
        **params: Any,
    ) -> dict[str, Any]:
        return self.client.turn_start(self.thread_id, input_items, **params)

    def turn_text(self, text: str, **params: Any) -> dict[str, Any]:
        return self.client.turn_text(self.thread_id, text, **params)

    def turn_text_schema(self, text: str, **params: Any) -> SchemaTurnStartResponse:
        return self.client.turn_text_schema(self.thread_id, text, **params)

    def ask(self, text: str, **params: Any) -> str:
        answer, _ = self.client.run_text_turn(self.thread_id, text, **params)
        return answer

    def stream(self, text: str, **params: Any) -> Iterator[Notification]:
        turn = self.turn_text(text, **params)
        turn_id = turn["turn"]["id"]
        while True:
            event = self.client.next_notification()
            yield event
            if event.method == "turn/completed" and (event.params or {}).get("turn", {}).get("id") == turn_id:
                break


@dataclass(slots=True)
class AsyncConversation:
    """Fluent thread-scoped helper over :class:`AsyncAppServerClient`."""

    client: "AsyncAppServerClient"
    thread_id: str

    async def turn_start(
        self,
        input_items: list[dict[str, Any]] | dict[str, Any] | str,
        **params: Any,
    ) -> dict[str, Any]:
        return await self.client.turn_start(self.thread_id, input_items, **params)

    async def turn_text(self, text: str, **params: Any) -> dict[str, Any]:
        return await self.client.turn_text(self.thread_id, text, **params)

    async def turn_text_schema(self, text: str, **params: Any) -> SchemaTurnStartResponse:
        return await self.client.turn_text_schema(self.thread_id, text, **params)

    async def ask(self, text: str, **params: Any) -> str:
        answer, _ = await self.client.run_text_turn(self.thread_id, text, **params)
        return answer

    async def stream(self, text: str, **params: Any) -> AsyncIterator[Notification]:
        turn = await self.turn_text(text, **params)
        turn_id = turn["turn"]["id"]
        while True:
            event = await self.client.next_notification()
            yield event
            if event.method == "turn/completed" and (event.params or {}).get("turn", {}).get("id") == turn_id:
                break
