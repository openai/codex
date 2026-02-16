from __future__ import annotations

import asyncio
from typing import Any, Iterable

from .client import AppServerClient, AppServerConfig
from .conversation import AsyncConversation
from .models import Notification
from .schema_types import (
    ThreadListResponse as SchemaThreadListResponse,
    ThreadStartResponse as SchemaThreadStartResponse,
    TurnStartResponse as SchemaTurnStartResponse,
)


class AsyncAppServerClient:
    """Async wrapper around AppServerClient using thread offloading.

    This keeps the public API notebook/async friendly while preserving a single
    battle-tested sync transport implementation.
    """

    def __init__(self, config: AppServerConfig | None = None):
        self._sync = AppServerClient(config=config)

    async def __aenter__(self) -> "AsyncAppServerClient":
        await self.start()
        return self

    async def __aexit__(self, exc_type, exc, tb) -> None:
        await self.close()

    async def start(self) -> None:
        await asyncio.to_thread(self._sync.start)

    async def close(self) -> None:
        await asyncio.to_thread(self._sync.close)

    async def initialize(self) -> dict[str, Any]:
        return await asyncio.to_thread(self._sync.initialize)

    async def thread_start(self, **params: Any) -> dict[str, Any]:
        return await asyncio.to_thread(self._sync.thread_start, **params)

    async def thread_resume(self, thread_id: str, **params: Any) -> dict[str, Any]:
        return await asyncio.to_thread(self._sync.thread_resume, thread_id, **params)

    async def thread_list(self, **params: Any) -> dict[str, Any]:
        return await asyncio.to_thread(self._sync.thread_list, **params)

    async def thread_read(self, thread_id: str, include_turns: bool = False) -> dict[str, Any]:
        return await asyncio.to_thread(self._sync.thread_read, thread_id, include_turns)

    async def turn_start(
        self,
        thread_id: str,
        input_items: list[dict[str, Any]] | dict[str, Any] | str,
        **params: Any,
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self._sync.turn_start, thread_id, input_items, **params)

    async def turn_text(self, thread_id: str, text: str, **params: Any) -> dict[str, Any]:
        return await asyncio.to_thread(self._sync.turn_text, thread_id, text, **params)

    async def turn_interrupt(self, thread_id: str, turn_id: str) -> dict[str, Any]:
        return await asyncio.to_thread(self._sync.turn_interrupt, thread_id, turn_id)

    async def model_list(self, include_hidden: bool = False) -> dict[str, Any]:
        return await asyncio.to_thread(self._sync.model_list, include_hidden)

    def conversation(self, thread_id: str) -> AsyncConversation:
        return AsyncConversation(client=self, thread_id=thread_id)

    async def conversation_start(self, *, model: str | None = None, **params: Any) -> AsyncConversation:
        payload = dict(params)
        if model is not None:
            payload["model"] = model
        started = await self.thread_start(**payload)
        return AsyncConversation(client=self, thread_id=started["thread"]["id"])

    async def thread_start_schema(self, **params: Any) -> SchemaThreadStartResponse:
        return await asyncio.to_thread(self._sync.thread_start_schema, **params)

    async def thread_list_schema(self, **params: Any) -> SchemaThreadListResponse:
        return await asyncio.to_thread(self._sync.thread_list_schema, **params)

    async def turn_start_schema(
        self,
        thread_id: str,
        input_items: list[dict[str, Any]] | dict[str, Any] | str,
        **params: Any,
    ) -> SchemaTurnStartResponse:
        return await asyncio.to_thread(self._sync.turn_start_schema, thread_id, input_items, **params)

    async def turn_text_schema(self, thread_id: str, text: str, **params: Any) -> SchemaTurnStartResponse:
        return await asyncio.to_thread(self._sync.turn_text_schema, thread_id, text, **params)

    async def next_notification(self) -> Notification:
        return await asyncio.to_thread(self._sync.next_notification)

    async def wait_for_turn_completed(self, turn_id: str) -> Notification:
        return await asyncio.to_thread(self._sync.wait_for_turn_completed, turn_id)

    async def stream_until_methods(self, methods: Iterable[str] | str) -> list[Notification]:
        return await asyncio.to_thread(self._sync.stream_until_methods, methods)

    async def run_text_turn(self, thread_id: str, text: str, **params: Any) -> tuple[str, Notification]:
        return await asyncio.to_thread(self._sync.run_text_turn, thread_id, text, **params)

    async def ask(self, text: str, *, model: str | None = None, thread_id: str | None = None) -> tuple[str, str]:
        return await asyncio.to_thread(self._sync.ask, text, model=model, thread_id=thread_id)
