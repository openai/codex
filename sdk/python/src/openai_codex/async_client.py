from __future__ import annotations

import asyncio
from collections.abc import Iterator
from typing import AsyncIterator, Callable, ParamSpec, TypeVar

from pydantic import BaseModel

from .client import AppServerClient, AppServerConfig
from .generated.v2_all import (
    AccountLoginCompletedNotification,
    AgentMessageDeltaNotification,
    CancelLoginAccountResponse,
    GetAccountParams as V2GetAccountParams,
    GetAccountResponse,
    LoginAccountParams as V2LoginAccountParams,
    LoginAccountResponse,
    LogoutAccountResponse,
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

    def __init__(self, config: AppServerConfig | None = None) -> None:
        """Create the wrapped sync client that owns the transport process."""
        self._sync = AppServerClient(config=config)

    async def __aenter__(self) -> "AsyncAppServerClient":
        """Start the app-server process when entering an async context."""
        await self.start()
        return self

    async def __aexit__(self, _exc_type, _exc, _tb) -> None:
        """Close the app-server process when leaving an async context."""
        await self.close()

    async def _call_sync(
        self,
        fn: Callable[ParamsT, ReturnT],
        /,
        *args: ParamsT.args,
        **kwargs: ParamsT.kwargs,
    ) -> ReturnT:
        """Run a blocking sync-client operation without blocking the event loop."""
        return await asyncio.to_thread(fn, *args, **kwargs)

    @staticmethod
    def _next_from_iterator(
        iterator: Iterator[AgentMessageDeltaNotification],
    ) -> tuple[bool, AgentMessageDeltaNotification | None]:
        """Convert StopIteration into a value that can cross asyncio.to_thread."""
        try:
            return True, next(iterator)
        except StopIteration:
            return False, None

    async def start(self) -> None:
        """Start the wrapped sync client in a worker thread."""
        await self._call_sync(self._sync.start)

    async def close(self) -> None:
        """Close the wrapped sync client in a worker thread."""
        await self._call_sync(self._sync.close)

    async def initialize(self) -> InitializeResponse:
        """Initialize the app-server session."""
        return await self._call_sync(self._sync.initialize)

    def register_turn_notifications(self, turn_id: str) -> None:
        """Register a turn notification queue on the wrapped sync client."""
        self._sync.register_turn_notifications(turn_id)

    def register_login_notifications(self, login_id: str) -> None:
        """Register a login notification queue on the wrapped sync client."""
        self._sync.register_login_notifications(login_id)

    def unregister_login_notifications(self, login_id: str) -> None:
        """Unregister a login notification queue on the wrapped sync client."""
        self._sync.unregister_login_notifications(login_id)

    def unregister_turn_notifications(self, turn_id: str) -> None:
        """Unregister a turn notification queue on the wrapped sync client."""
        self._sync.unregister_turn_notifications(turn_id)

    async def request(
        self,
        method: str,
        params: JsonObject | None,
        *,
        response_model: type[ModelT],
    ) -> ModelT:
        """Send a typed JSON-RPC request through the wrapped sync client."""
        return await self._call_sync(
            self._sync.request,
            method,
            params,
            response_model=response_model,
        )

    async def account_login_start(
        self,
        params: V2LoginAccountParams | JsonObject,
    ) -> LoginAccountResponse:
        """Start one account login attempt through the wrapped sync client."""
        return await self._call_sync(self._sync.account_login_start, params)

    async def account_login_cancel(self, login_id: str) -> CancelLoginAccountResponse:
        """Cancel one active account login attempt through the wrapped sync client."""
        return await self._call_sync(self._sync.account_login_cancel, login_id)

    async def account_read(
        self,
        params: V2GetAccountParams | JsonObject | None = None,
    ) -> GetAccountResponse:
        """Read current account state through the wrapped sync client."""
        return await self._call_sync(self._sync.account_read, params)

    async def account_logout(self) -> LogoutAccountResponse:
        """Clear the active account session through the wrapped sync client."""
        return await self._call_sync(self._sync.account_logout)

    async def thread_start(
        self, params: V2ThreadStartParams | JsonObject | None = None
    ) -> ThreadStartResponse:
        """Start a thread using the wrapped sync client."""
        return await self._call_sync(self._sync.thread_start, params)

    async def thread_resume(
        self,
        thread_id: str,
        params: V2ThreadResumeParams | JsonObject | None = None,
    ) -> ThreadResumeResponse:
        """Resume a thread using the wrapped sync client."""
        return await self._call_sync(self._sync.thread_resume, thread_id, params)

    async def thread_list(
        self, params: V2ThreadListParams | JsonObject | None = None
    ) -> ThreadListResponse:
        """List threads using the wrapped sync client."""
        return await self._call_sync(self._sync.thread_list, params)

    async def thread_read(self, thread_id: str, include_turns: bool = False) -> ThreadReadResponse:
        """Read a thread using the wrapped sync client."""
        return await self._call_sync(self._sync.thread_read, thread_id, include_turns)

    async def thread_fork(
        self,
        thread_id: str,
        params: V2ThreadForkParams | JsonObject | None = None,
    ) -> ThreadForkResponse:
        """Fork a thread using the wrapped sync client."""
        return await self._call_sync(self._sync.thread_fork, thread_id, params)

    async def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:
        """Archive a thread using the wrapped sync client."""
        return await self._call_sync(self._sync.thread_archive, thread_id)

    async def thread_unarchive(self, thread_id: str) -> ThreadUnarchiveResponse:
        """Unarchive a thread using the wrapped sync client."""
        return await self._call_sync(self._sync.thread_unarchive, thread_id)

    async def thread_set_name(self, thread_id: str, name: str) -> ThreadSetNameResponse:
        """Rename a thread using the wrapped sync client."""
        return await self._call_sync(self._sync.thread_set_name, thread_id, name)

    async def thread_compact(self, thread_id: str) -> ThreadCompactStartResponse:
        """Start thread compaction using the wrapped sync client."""
        return await self._call_sync(self._sync.thread_compact, thread_id)

    async def turn_start(
        self,
        thread_id: str,
        input_items: list[JsonObject] | JsonObject | str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TurnStartResponse:
        """Start a turn using the wrapped sync client."""
        return await self._call_sync(self._sync.turn_start, thread_id, input_items, params)

    async def turn_interrupt(self, thread_id: str, turn_id: str) -> TurnInterruptResponse:
        """Interrupt a turn using the wrapped sync client."""
        return await self._call_sync(self._sync.turn_interrupt, thread_id, turn_id)

    async def turn_steer(
        self,
        thread_id: str,
        expected_turn_id: str,
        input_items: list[JsonObject] | JsonObject | str,
    ) -> TurnSteerResponse:
        """Send steering input to a turn using the wrapped sync client."""
        return await self._call_sync(
            self._sync.turn_steer,
            thread_id,
            expected_turn_id,
            input_items,
        )

    async def model_list(self, include_hidden: bool = False) -> ModelListResponse:
        """List models using the wrapped sync client."""
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
        """Send a typed request with the sync client's overload retry policy."""
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
        """Wait for the next global notification without blocking the event loop."""
        return await self._call_sync(self._sync.next_notification)

    async def next_login_notification(self, login_id: str) -> Notification:
        """Wait for the next notification routed to one login attempt."""
        return await self._call_sync(self._sync.next_login_notification, login_id)

    async def next_turn_notification(self, turn_id: str) -> Notification:
        """Wait for the next notification routed to one turn."""
        return await self._call_sync(self._sync.next_turn_notification, turn_id)

    async def wait_for_login_completed(
        self,
        login_id: str,
    ) -> AccountLoginCompletedNotification:
        """Wait for the completion notification routed to one login attempt."""
        return await self._call_sync(self._sync.wait_for_login_completed, login_id)

    async def wait_for_turn_completed(self, turn_id: str) -> TurnCompletedNotification:
        """Wait for the completion notification routed to one turn."""
        return await self._call_sync(self._sync.wait_for_turn_completed, turn_id)

    async def stream_text(
        self,
        thread_id: str,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> AsyncIterator[AgentMessageDeltaNotification]:
        """Stream text deltas from one turn without monopolizing the event loop."""
        iterator = self._sync.stream_text(thread_id, text, params)
        while True:
            has_value, chunk = await asyncio.to_thread(
                self._next_from_iterator,
                iterator,
            )
            if not has_value:
                break
            yield chunk
