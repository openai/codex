from __future__ import annotations

import asyncio
from typing import Any

from .client import AppServerClient, AppServerConfig
from .models import Notification


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

    async def turn_start(self, thread_id: str, input_items: list[dict[str, Any]], **params: Any) -> dict[str, Any]:
        return await asyncio.to_thread(self._sync.turn_start, thread_id, input_items, **params)

    async def wait_for_turn_completed(self, turn_id: str) -> Notification:
        return await asyncio.to_thread(self._sync.wait_for_turn_completed, turn_id)

    async def stream_until_methods(self, methods: set[str]) -> list[Notification]:
        return await asyncio.to_thread(self._sync.stream_until_methods, methods)
