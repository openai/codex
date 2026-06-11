import asyncio
import queue
import threading
import time
from collections import deque
from dataclasses import dataclass, field
from typing import AsyncIterator, Awaitable, Callable, Iterator

from .generated.notification_registry import notification_turn_id
from .generated.v2_all import (
    ThreadGoalClearedNotification,
    ThreadGoalStatus,
    ThreadGoalUpdatedNotification,
    Turn,
    TurnCompletedNotification,
    TurnStartedNotification,
    TurnStatus,
)
from .models import Notification, UnknownNotification


class _GoalStreamClosed(Exception):
    """Wake a notification reader after its logical stream closes."""


def _terminal_goal_status(status: ThreadGoalStatus | None) -> bool:
    return status in {
        ThreadGoalStatus.paused,
        ThreadGoalStatus.blocked,
        ThreadGoalStatus.usage_limited,
        ThreadGoalStatus.budget_limited,
        ThreadGoalStatus.complete,
    }


@dataclass(slots=True)
class _GoalOperationState:
    """Private state for one goal operation exposed as a logical turn."""

    thread_id: str
    logical_turn_id: str | None = None
    current_turn_id: str | None = None
    status: ThreadGoalStatus | None = None
    started_turn: Turn | None = None
    completed_turn: Turn | None = None
    interrupted: bool = False
    interrupt_requested: bool = False
    cleared: bool = False
    _condition: threading.Condition = field(default_factory=threading.Condition)
    _notifications: queue.Queue[Notification | BaseException] = field(default_factory=queue.Queue)
    _failure: BaseException | None = None
    _finished: bool = False
    _turn_routing_active: bool = False

    def observe(self, notification: Notification) -> bool:
        payload = notification.payload
        with self._condition:
            if not self._turn_routing_active and not isinstance(
                payload,
                ThreadGoalClearedNotification | ThreadGoalUpdatedNotification,
            ):
                return False
            if isinstance(payload, TurnStartedNotification):
                if self.logical_turn_id is None:
                    self.logical_turn_id = payload.turn.id
                self.current_turn_id = payload.turn.id
                if self.started_turn is None:
                    self.started_turn = payload.turn
            elif isinstance(payload, TurnCompletedNotification):
                self.completed_turn = payload.turn
                if self.current_turn_id == payload.turn.id:
                    self.current_turn_id = None
            elif isinstance(payload, ThreadGoalUpdatedNotification):
                self.status = payload.goal.status
                if self.status == ThreadGoalStatus.active:
                    self.cleared = False
            elif isinstance(payload, ThreadGoalClearedNotification):
                self.cleared = True
            if (
                self.current_turn_id is None
                and self.completed_turn is not None
                and (self.cleared or _terminal_goal_status(self.status))
            ):
                self._finished = True
            self._condition.notify_all()
        self._notifications.put(notification)
        return True

    def activate_turn_routing(self) -> None:
        """Accept physical turns after the previous stored goal is cleared."""
        with self._condition:
            self._turn_routing_active = True

    def wait_for_start(self, timeout: float) -> str | None:
        """Wait for the runtime-generated first turn without consuming its event."""
        deadline = time.monotonic() + timeout
        with self._condition:
            while self.started_turn is None or self.logical_turn_id is None:
                if self._failure is not None:
                    raise self._failure
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    return None
                self._condition.wait(remaining)
            return self.logical_turn_id

    def fail(self, exc: BaseException) -> None:
        with self._condition:
            self._failure = exc
            self._condition.notify_all()
        self._notifications.put(exc)

    def next_notification(self) -> Notification:
        item = self._notifications.get()
        if isinstance(item, BaseException):
            raise item
        return item

    def finish(self) -> None:
        """Mark the logical operation inactive and wake waiting controls."""
        with self._condition:
            self._finished = True
            self.current_turn_id = None
            self._condition.notify_all()

    def is_finished(self) -> bool:
        with self._condition:
            return self._finished

    def begin_interrupt(self) -> bool:
        with self._condition:
            if self._finished:
                return False
            self.interrupt_requested = True
            return True

    def confirm_interrupt(self) -> None:
        with self._condition:
            self.interrupted = True
            self.interrupt_requested = False
            self._condition.notify_all()

    def cancel_interrupt(self) -> None:
        with self._condition:
            self.interrupt_requested = False
            self._condition.notify_all()

    def explicit_interrupt(self) -> bool:
        with self._condition:
            while self.interrupt_requested:
                self._condition.wait()
            return self.interrupted

    def active_turn(self, *, after: str | None = None) -> str | None:
        """Wait for the current turn, or return None once the goal has ended."""
        with self._condition:
            while True:
                if self._failure is not None:
                    raise self._failure
                if self._finished:
                    return None
                if self.current_turn_id is not None and self.current_turn_id != after:
                    return self.current_turn_id
                if self.cleared or _terminal_goal_status(self.status):
                    return None
                self._condition.wait()

    def current_turn(self) -> str | None:
        """Return the current physical turn without waiting for rollover."""
        with self._condition:
            return self.current_turn_id

    def resolve_active_turn(self, expected: str, active: str) -> None:
        """Adopt a server-reported active id when routed state is still stale."""
        with self._condition:
            if self.current_turn_id in {None, expected}:
                self.current_turn_id = active
                self._condition.notify_all()

    def turn_for_interrupt(self) -> str | None:
        """Return an active or stale turn id that can resolve rollover races."""
        with self._condition:
            if self.current_turn_id is not None:
                return self.current_turn_id
            if self.completed_turn is not None:
                return self.completed_turn.id
            if self.started_turn is not None:
                return self.started_turn.id
            return None

    def wake_notification_reader(self) -> None:
        """Release a reader blocked after its stream has been closed."""
        self._notifications.put(_GoalStreamClosed())


def _logical_notification(notification: Notification, logical_turn_id: str) -> Notification:
    """Return a copy whose turn metadata uses the logical operation id."""
    payload = notification.payload
    if isinstance(payload, UnknownNotification):
        params = dict(payload.params)
        if isinstance(params.get("turnId"), str):
            params["turnId"] = logical_turn_id
        turn = params.get("turn")
        if isinstance(turn, dict) and isinstance(turn.get("id"), str):
            params["turn"] = {**turn, "id": logical_turn_id}
        return Notification(notification.method, UnknownNotification(params))

    turn_id = notification_turn_id(payload)
    if turn_id is None:
        return notification
    if hasattr(payload, "turn_id"):
        return Notification(
            notification.method,
            payload.model_copy(update={"turn_id": logical_turn_id}),
        )
    if hasattr(payload, "turn"):
        logical_turn = payload.turn.model_copy(update={"id": logical_turn_id})
        return Notification(
            notification.method,
            payload.model_copy(update={"turn": logical_turn}),
        )
    return notification


def _logical_completion(
    completed: TurnCompletedNotification,
    *,
    logical_turn_id: str,
    started: Turn | None,
    interrupted: bool,
) -> TurnCompletedNotification:
    """Coalesce the final physical completion into one logical completion."""
    final_turn = completed.turn
    started_at = started.started_at if started is not None else final_turn.started_at
    duration_ms = final_turn.duration_ms
    if started_at is not None and final_turn.completed_at is not None:
        duration_ms = max(0, final_turn.completed_at - started_at) * 1000
    updates: dict[str, object] = {
        "id": logical_turn_id,
        "started_at": started_at,
        "duration_ms": duration_ms,
    }
    if interrupted:
        updates["status"] = TurnStatus.interrupted
    return completed.model_copy(update={"turn": final_turn.model_copy(update=updates)})


@dataclass(slots=True)
class _GoalStreamCursor:
    """Consume physical goal events as one ordered logical turn stream."""

    state: _GoalOperationState
    started: Turn | None = None
    last_completed: TurnCompletedNotification | None = None
    failed_completion: TurnCompletedNotification | None = None
    status: ThreadGoalStatus | None = None
    active: bool = False
    cleared: bool = False

    def process(self, notification: Notification) -> tuple[list[Notification], bool]:
        logical_turn_id = self.state.logical_turn_id
        if logical_turn_id is None:
            raise RuntimeError("goal operation has not been bound to a logical turn id")

        payload = notification.payload
        if isinstance(payload, TurnStartedNotification):
            self.active = True
            if self.started is not None:
                return [], False
            self.started = payload.turn
            return [_logical_notification(notification, logical_turn_id)], False

        if isinstance(payload, TurnCompletedNotification):
            self.active = False
            self.last_completed = payload
            if payload.turn.status == TurnStatus.interrupted:
                return [
                    self._completion(
                        notification.method,
                        self.failed_completion or payload,
                    )
                ], True
            if payload.turn.status == TurnStatus.failed:
                self.failed_completion = payload
                if self.cleared or _terminal_goal_status(self.status):
                    self.state.finish()
                    return [self._completion(notification.method, payload)], True
                return [], False
            if self.status is None and not self.cleared:
                raise RuntimeError(
                    "the connected Codex runtime did not activate goal mode for this turn"
                )
            if self.cleared or _terminal_goal_status(self.status):
                self.state.finish()
                return [
                    self._completion(
                        notification.method,
                        self.failed_completion or payload,
                    )
                ], True
            return [], False

        events = [_logical_notification(notification, logical_turn_id)]
        if isinstance(payload, ThreadGoalUpdatedNotification):
            self.status = payload.goal.status
            if self.status == ThreadGoalStatus.active:
                self.cleared = False
            events = []
        elif isinstance(payload, ThreadGoalClearedNotification):
            self.cleared = True
            events = []

        if (
            not self.active
            and self.last_completed is not None
            and (self.cleared or _terminal_goal_status(self.status))
        ):
            self.state.finish()
            events.append(
                self._completion(
                    "turn/completed",
                    self.failed_completion or self.last_completed,
                )
            )
            return events, True
        return events, False

    def _completion(
        self,
        method: str,
        payload: TurnCompletedNotification,
    ) -> Notification:
        logical_turn_id = self.state.logical_turn_id
        if logical_turn_id is None:
            raise RuntimeError("goal operation has not been bound to a logical turn id")
        return Notification(
            method,
            _logical_completion(
                payload,
                logical_turn_id=logical_turn_id,
                started=self.started,
                interrupted=self.state.explicit_interrupt(),
            ),
        )


@dataclass(slots=True)
class _GoalNotificationStream(Iterator[Notification]):
    """Closeable synchronous view of one logical goal operation."""

    state: _GoalOperationState
    next_notification: Callable[[], Notification]
    unregister: Callable[[], None]
    cancel_goal: Callable[[], None]
    _cursor: _GoalStreamCursor = field(init=False)
    _pending: deque[Notification] = field(default_factory=deque)
    _closed: bool = False

    def __post_init__(self) -> None:
        self._cursor = _GoalStreamCursor(self.state)

    def __iter__(self) -> "_GoalNotificationStream":
        return self

    def __next__(self) -> Notification:
        if self._closed:
            raise StopIteration
        try:
            while not self._pending:
                notification = self.next_notification()
                events, completed = self._cursor.process(notification)
                self._pending.extend(events)
                if completed:
                    self._finish()
            return self._pending.popleft()
        except _GoalStreamClosed:
            self.close()
            raise StopIteration from None
        except KeyboardInterrupt:
            self.cancel_goal()
            self.close()
            raise
        except BaseException:
            self.close()
            raise

    def _finish(self) -> None:
        if self._closed:
            return
        self.state.finish()
        self.state.wake_notification_reader()
        self.unregister()
        self._closed = True

    def close(self) -> None:
        self._finish()


@dataclass(slots=True)
class _AsyncGoalNotificationStream(AsyncIterator[Notification]):
    """Closeable asynchronous view of one logical goal operation."""

    state: _GoalOperationState
    next_notification: Callable[[], Awaitable[Notification]]
    unregister: Callable[[], None]
    cancel_goal: Callable[[], Awaitable[None]]
    _cursor: _GoalStreamCursor = field(init=False)
    _pending: deque[Notification] = field(default_factory=deque)
    _closed: bool = False

    def __post_init__(self) -> None:
        self._cursor = _GoalStreamCursor(self.state)

    def __aiter__(self) -> "_AsyncGoalNotificationStream":
        return self

    async def __anext__(self) -> Notification:
        if self._closed:
            raise StopAsyncIteration
        try:
            while not self._pending:
                notification = await self.next_notification()
                events, completed = self._cursor.process(notification)
                self._pending.extend(events)
                if completed:
                    self._finish()
            return self._pending.popleft()
        except _GoalStreamClosed:
            await self.aclose()
            raise StopAsyncIteration from None
        except asyncio.CancelledError:
            await self.cancel_goal()
            await self.aclose()
            raise
        except BaseException:
            await self.aclose()
            raise

    def _finish(self) -> None:
        if self._closed:
            return
        self.state.finish()
        self.state.wake_notification_reader()
        self.unregister()
        self._closed = True

    async def aclose(self) -> None:
        self._finish()
