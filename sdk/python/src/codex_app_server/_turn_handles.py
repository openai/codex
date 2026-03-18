from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, AsyncIterator, Iterator

from .client import AppServerClient
from .generated.v2_all import (
    Turn as AppServerTurn,
    TurnCompletedNotification,
    TurnInterruptResponse,
    TurnSteerResponse,
)
from .models import Notification
from ._inputs import Input, _to_wire_input

if TYPE_CHECKING:
    from .api import AsyncCodex


@dataclass(slots=True)
class TurnHandle:
    _client: AppServerClient
    thread_id: str
    id: str

    def steer(self, input: Input) -> TurnSteerResponse:
        return self._client.turn_steer(self.thread_id, self.id, _to_wire_input(input))

    def interrupt(self) -> TurnInterruptResponse:
        return self._client.turn_interrupt(self.thread_id, self.id)

    def stream(self) -> Iterator[Notification]:
        # TODO: replace this client-wide experimental guard with per-turn event demux.
        self._client.acquire_turn_consumer(self.id)
        try:
            while True:
                event = self._client.next_notification()
                yield event
                if (
                    event.method == "turn/completed"
                    and isinstance(event.payload, TurnCompletedNotification)
                    and event.payload.turn.id == self.id
                ):
                    break
        finally:
            self._client.release_turn_consumer(self.id)

    def run(self) -> AppServerTurn:
        completed: TurnCompletedNotification | None = None
        stream = self.stream()
        try:
            for event in stream:
                payload = event.payload
                if isinstance(payload, TurnCompletedNotification) and payload.turn.id == self.id:
                    completed = payload
        finally:
            stream.close()

        if completed is None:
            raise RuntimeError("turn completed event not received")
        return completed.turn


@dataclass(slots=True)
class AsyncTurnHandle:
    _codex: AsyncCodex
    thread_id: str
    id: str

    async def steer(self, input: Input) -> TurnSteerResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.turn_steer(
            self.thread_id,
            self.id,
            _to_wire_input(input),
        )

    async def interrupt(self) -> TurnInterruptResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.turn_interrupt(self.thread_id, self.id)

    async def stream(self) -> AsyncIterator[Notification]:
        await self._codex._ensure_initialized()
        # TODO: replace this client-wide experimental guard with per-turn event demux.
        self._codex._client.acquire_turn_consumer(self.id)
        try:
            while True:
                event = await self._codex._client.next_notification()
                yield event
                if (
                    event.method == "turn/completed"
                    and isinstance(event.payload, TurnCompletedNotification)
                    and event.payload.turn.id == self.id
                ):
                    break
        finally:
            self._codex._client.release_turn_consumer(self.id)

    async def run(self) -> AppServerTurn:
        completed: TurnCompletedNotification | None = None
        stream = self.stream()
        try:
            async for event in stream:
                payload = event.payload
                if isinstance(payload, TurnCompletedNotification) and payload.turn.id == self.id:
                    completed = payload
        finally:
            await stream.aclose()

        if completed is None:
            raise RuntimeError("turn completed event not received")
        return completed.turn
