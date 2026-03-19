from __future__ import annotations

import asyncio
from collections.abc import Iterator
from typing import AsyncIterator, Callable, Iterable, ParamSpec, TypeVar

from pydantic import BaseModel

from .client import (
    AppServerClient,
    AppServerConfig,
    ApprovalHandler,
    ModelCompactHandler,
    ModelRequestHandler,
)
from .generated.v2_all import (
    AgentMessageDeltaNotification,
    ModelListResponse,
    ThreadArchiveResponse,
    ThreadCompactStartResponse,
    ThreadForkParams as V2ThreadForkParams,
    ThreadForkResponse,
    ThreadListParams as V2ThreadListParams,
    ThreadListResponse,
    ThreadReadResponse,
    ThreadResumeParams as V2ThreadResumeParams,
    ThreadResumeResponse,
    ThreadSetNameResponse,
    ThreadStartParams as V2ThreadStartParams,
    ThreadStartResponse,
    ThreadUnarchiveResponse,
    TurnCompletedNotification,
    TurnInterruptResponse,
    TurnStartParams as V2TurnStartParams,
    TurnStartResponse,
    TurnSteerResponse,
)
from .models import InitializeResponse, JsonObject, Notification

ModelT = TypeVar("ModelT", bound=BaseModel)
ParamsT = ParamSpec("ParamsT")
ReturnT = TypeVar("ReturnT")


class AsyncAppServerClient:
    """Async wrapper around AppServerClient using thread offloading."""

    def __init__(
        self,
        config: AppServerConfig | None = None,
        approval_handler: ApprovalHandler | None = None,
        model_request_handler: ModelRequestHandler | None = None,
        model_compact_handler: ModelCompactHandler | None = None,
    ) -> None:
        self._sync = AppServerClient(
            config=config,
            approval_handler=approval_handler,
            model_request_handler=model_request_handler,
            model_compact_handler=model_compact_handler,
        )
        # Single stdio transport cannot be read safely from multiple threads.
        self._transport_lock = asyncio.Lock()

    async def __aenter__(self) -> "AsyncAppServerClient":
        await self.start()
        return self

    async def __aexit__(self, _exc_type, _exc, _tb) -> None:
        await self.close()

    async def _call_sync(
        self,
        fn: Callable[ParamsT, ReturnT],
        /,
        *args: ParamsT.args,
        **kwargs: ParamsT.kwargs,
    ) -> ReturnT:
        async with self._transport_lock:
            return await asyncio.to_thread(fn, *args, **kwargs)

    @staticmethod
    def _next_from_iterator(
        iterator: Iterator[AgentMessageDeltaNotification],
    ) -> tuple[bool, AgentMessageDeltaNotification | None]:
        try:
            return True, next(iterator)
        except StopIteration:
            return False, None

    async def start(self) -> None:
        await self._call_sync(self._sync.start)

    async def close(self) -> None:
        await self._call_sync(self._sync.close)

    async def initialize(self) -> InitializeResponse:
        return await self._call_sync(self._sync.initialize)

    def acquire_turn_consumer(self, turn_id: str) -> None:
        self._sync.acquire_turn_consumer(turn_id)

    def release_turn_consumer(self, turn_id: str) -> None:
        self._sync.release_turn_consumer(turn_id)

    async def request(
        self,
        method: str,
        params: JsonObject | None,
        *,
        response_model: type[ModelT],
    ) -> ModelT:
        return await self._call_sync(
            self._sync.request,
            method,
            params,
            response_model=response_model,
        )

    async def send_model_stream_metadata(
        self,
        *,
        thread_id: str,
        turn_id: str,
        request_id: str,
        metadata: dict[str, str],
    ) -> None:
        await self._call_sync(
            self._sync.send_model_stream_metadata,
            thread_id=thread_id,
            turn_id=turn_id,
            request_id=request_id,
            metadata=metadata,
        )

    async def send_model_stream_event(
        self,
        *,
        thread_id: str,
        turn_id: str,
        request_id: str,
        event: JsonObject,
    ) -> None:
        await self._call_sync(
            self._sync.send_model_stream_event,
            thread_id=thread_id,
            turn_id=turn_id,
            request_id=request_id,
            event=event,
        )

    async def send_model_request_failed(
        self,
        *,
        thread_id: str,
        turn_id: str,
        request_id: str,
        error: JsonObject,
    ) -> None:
        await self._call_sync(
            self._sync.send_model_request_failed,
            thread_id=thread_id,
            turn_id=turn_id,
            request_id=request_id,
            error=error,
        )

    async def thread_start(
        self, params: V2ThreadStartParams | JsonObject | None = None
    ) -> ThreadStartResponse:
        return await self._call_sync(self._sync.thread_start, params)

    async def thread_resume(
        self,
        thread_id: str,
        params: V2ThreadResumeParams | JsonObject | None = None,
    ) -> ThreadResumeResponse:
        return await self._call_sync(self._sync.thread_resume, thread_id, params)

    async def thread_list(
        self, params: V2ThreadListParams | JsonObject | None = None
    ) -> ThreadListResponse:
        return await self._call_sync(self._sync.thread_list, params)

    async def thread_read(
        self, thread_id: str, include_turns: bool = False
    ) -> ThreadReadResponse:
        return await self._call_sync(self._sync.thread_read, thread_id, include_turns)

    async def thread_fork(
        self,
        thread_id: str,
        params: V2ThreadForkParams | JsonObject | None = None,
    ) -> ThreadForkResponse:
        return await self._call_sync(self._sync.thread_fork, thread_id, params)

    async def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:
        return await self._call_sync(self._sync.thread_archive, thread_id)

    async def thread_unarchive(self, thread_id: str) -> ThreadUnarchiveResponse:
        return await self._call_sync(self._sync.thread_unarchive, thread_id)

    async def thread_set_name(self, thread_id: str, name: str) -> ThreadSetNameResponse:
        return await self._call_sync(self._sync.thread_set_name, thread_id, name)

    async def thread_compact(self, thread_id: str) -> ThreadCompactStartResponse:
        return await self._call_sync(self._sync.thread_compact, thread_id)

    async def turn_start(
        self,
        thread_id: str,
        input_items: list[JsonObject] | JsonObject | str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TurnStartResponse:
        return await self._call_sync(
            self._sync.turn_start, thread_id, input_items, params
        )

    async def turn_interrupt(
        self, thread_id: str, turn_id: str
    ) -> TurnInterruptResponse:
        return await self._call_sync(self._sync.turn_interrupt, thread_id, turn_id)

    async def turn_steer(
        self,
        thread_id: str,
        expected_turn_id: str,
        input_items: list[JsonObject] | JsonObject | str,
    ) -> TurnSteerResponse:
        return await self._call_sync(
            self._sync.turn_steer,
            thread_id,
            expected_turn_id,
            input_items,
        )

    async def model_list(self, include_hidden: bool = False) -> ModelListResponse:
        return await self._call_sync(self._sync.model_list, include_hidden)

    async def request_with_retry_on_overload(
        self,
        method: str,
        params: JsonObject | None,
        *,
        response_model: type[ModelT],
        max_attempts: int = 3,
        initial_delay_s: float = 0.25,
        max_delay_s: float = 2.0,
    ) -> ModelT:
        return await self._call_sync(
            self._sync.request_with_retry_on_overload,
            method,
            params,
            response_model=response_model,
            max_attempts=max_attempts,
            initial_delay_s=initial_delay_s,
            max_delay_s=max_delay_s,
        )

    async def next_notification(self) -> Notification:
        return await self._call_sync(self._sync.next_notification)

    async def wait_for_turn_completed(self, turn_id: str) -> TurnCompletedNotification:
        return await self._call_sync(self._sync.wait_for_turn_completed, turn_id)

    async def stream_until_methods(
        self, methods: Iterable[str] | str
    ) -> list[Notification]:
        return await self._call_sync(self._sync.stream_until_methods, methods)

    async def stream_text(
        self,
        thread_id: str,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> AsyncIterator[AgentMessageDeltaNotification]:
        async with self._transport_lock:
            iterator = self._sync.stream_text(thread_id, text, params)
            while True:
                has_value, chunk = await asyncio.to_thread(
                    self._next_from_iterator,
                    iterator,
                )
                if not has_value:
                    break
                yield chunk
