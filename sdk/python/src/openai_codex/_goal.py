import queue
import threading
import time
from dataclasses import dataclass, field

from .generated.v2_all import (
    ThreadGoalClearedNotification,
    ThreadGoalStatus,
    ThreadGoalUpdatedNotification,
    Turn,
    TurnCompletedNotification,
    TurnStartedNotification,
)
from .models import Notification


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

    def observe(self, notification: Notification) -> None:
        payload = notification.payload
        with self._condition:
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

    def cancel_interrupt(self) -> None:
        with self._condition:
            self.interrupt_requested = False

    def explicit_interrupt(self, status: ThreadGoalStatus | None) -> bool:
        with self._condition:
            return self.interrupted or (
                self.interrupt_requested and status == ThreadGoalStatus.paused
            )

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

    def wake_notification_reader(self) -> None:
        """Release a reader blocked after its stream has been closed."""
        self._notifications.put(_GoalStreamClosed())
