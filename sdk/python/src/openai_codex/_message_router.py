from __future__ import annotations

import queue
import threading
import weakref
from collections import deque
from contextlib import contextmanager
from dataclasses import dataclass, field
from typing import Iterator

from ._goal import _GoalOperationState
from .errors import CodexError, TransportClosedError, map_jsonrpc_error
from .generated.notification_registry import notification_turn_id
from .generated.v2_all import AccountLoginCompletedNotification
from .models import JsonValue, Notification, UnknownNotification

ResponseQueueItem = JsonValue | BaseException
NotificationQueueItem = Notification | BaseException


@dataclass
class _TurnState:
    id: str
    thread_id: str | None = None
    events: dict[int, NotificationQueueItem] = field(default_factory=dict)
    first_event: int = 0
    next_event: int = 0
    subscribers: dict[object, int] = field(default_factory=dict)
    completed: bool = False


class _TurnSubscription:
    """One consumer's cursor over shared unread events."""

    def __init__(self, router: MessageRouter, state: _TurnState, cursor: int) -> None:
        self._router = router
        self._state = state
        self._cursor = cursor
        self._token = object()
        state.subscribers[self._token] = self._cursor
        self._closed = False
        self._release = weakref.finalize(
            self, router._release_turn, weakref.ref(router), state, self._token
        )

    def next(self) -> Notification:
        with self._router._turn_condition:
            while self._cursor == self._state.next_event and not self._closed:
                if self._state.completed:
                    raise TransportClosedError("Turn is no longer streaming")
                self._router._turn_condition.wait()
            if self._closed:
                raise TransportClosedError("Turn subscription closed")
            item = self._state.events[self._cursor]
            self._cursor += 1
            self._state.subscribers[self._token] = self._cursor
            self._router._prune_turn_events(self._state)
        if isinstance(item, BaseException):
            raise item
        return item

    def close(self) -> None:
        with self._router._turn_condition:
            self._closed = True
            self._router._turn_condition.notify_all()
        self._release()


class MessageRouter:
    """Route reader-thread messages to the SDK operation waiting for them.

    The app-server stdio transport is a single ordered stream, so only the
    reader thread should consume stdout. This router keeps the rest of the SDK
    from competing for that stream by giving each in-flight JSON-RPC request
    its own queue and each turn consumer its own event cursor.
    """

    def __init__(self) -> None:
        """Create empty response, turn, and global notification queues."""
        # GC can release abandoned subscriptions during another routing operation.
        self._lock = threading.RLock()
        self._response_waiters: dict[str, queue.Queue[ResponseQueueItem]] = {}
        self._login_notifications: dict[str, queue.Queue[NotificationQueueItem]] = {}
        self._pending_login_notifications: dict[str, deque[Notification]] = {}
        self._turn_condition = threading.Condition(self._lock)
        self._turn_states: dict[str, _TurnState] = {}
        self._turn_notifications: dict[str, _TurnSubscription] = {}
        self._pending_turn_requests: dict[str, BaseException | None] = {}
        self._goal_operations: dict[str, _GoalOperationState] = {}
        self._global_notifications: queue.Queue[NotificationQueueItem] = queue.Queue()

    def create_response_waiter(self, request_id: str) -> queue.Queue[ResponseQueueItem]:
        """Register a one-shot queue for a JSON-RPC response id."""

        waiter: queue.Queue[ResponseQueueItem] = queue.Queue(maxsize=1)
        with self._lock:
            self._response_waiters[request_id] = waiter
        return waiter

    def discard_response_waiter(self, request_id: str) -> None:
        """Remove a response waiter when the request could not be written."""

        with self._lock:
            self._response_waiters.pop(request_id, None)

    def next_global_notification(self) -> Notification:
        """Block until the next notification that is not scoped to a turn."""

        item = self._global_notifications.get()
        if isinstance(item, BaseException):
            raise item
        return item

    def register_login(self, login_id: str) -> None:
        """Register a queue for one interactive login attempt."""

        login_queue: queue.Queue[NotificationQueueItem] = queue.Queue()
        with self._lock:
            if login_id in self._login_notifications:
                return
            pending = self._pending_login_notifications.pop(login_id, deque())
            self._login_notifications[login_id] = login_queue
        for notification in pending:
            login_queue.put(notification)

    def unregister_login(self, login_id: str) -> None:
        """Stop routing future notifications for one login attempt."""

        with self._lock:
            self._login_notifications.pop(login_id, None)

    def next_login_notification(self, login_id: str) -> Notification:
        """Block until the next notification for a registered login attempt."""

        with self._lock:
            login_queue = self._login_notifications.get(login_id)
        if login_queue is None:
            raise RuntimeError(f"login {login_id!r} is not registered for waiting")
        item = login_queue.get()
        if isinstance(item, BaseException):
            raise item
        return item

    @contextmanager
    def pending_turn(self, thread_id: str) -> Iterator[dict[str, int]]:
        """Buffer events from the point a turn/start request is sent."""
        with self._lock:
            cursors = {turn_id: state.next_event for turn_id, state in self._turn_states.items()}
            self._pending_turn_requests[thread_id] = None
        try:
            yield cursors
        finally:
            with self._lock:
                del self._pending_turn_requests[thread_id]
                for state in list(self._turn_states.values()):
                    if state.thread_id in (None, thread_id):
                        self._prune_turn_events(state)

    def prepare_turn(
        self, turn_id: str, thread_id: str, cursors: dict[str, int], *, for_handle: bool
    ) -> _TurnSubscription | None:
        """Attach the requesting handle or the single low-level consumer."""
        with self._lock:
            state = self._turn_states.setdefault(turn_id, _TurnState(turn_id, thread_id))
            state.thread_id = thread_id
            if not for_handle and turn_id in self._turn_notifications:
                return None
            if not state.completed and (err := self._pending_turn_requests[thread_id]) is not None:
                state.events[state.next_event] = err
                state.next_event += 1
                state.completed = True
            subscription = _TurnSubscription(self, state, cursors.get(turn_id, 0))
            if not for_handle:
                self._turn_notifications[turn_id] = subscription
            return subscription

    def subscribe_turn(self, turn_id: str) -> _TurnSubscription:
        """Attach a consumer starting at the next event for this turn."""
        with self._lock:
            state = self._turn_states.setdefault(turn_id, _TurnState(turn_id))
            return _TurnSubscription(self, state, state.next_event)

    @staticmethod
    def _release_turn(
        router_ref: weakref.ReferenceType[MessageRouter], state: _TurnState, token: object
    ) -> None:
        router = router_ref()
        if router is not None:
            with router._lock:
                default = router._turn_notifications.get(state.id)
                if default is not None and default._token is token:
                    del router._turn_notifications[state.id]
                state.subscribers.pop(token, None)
                router._prune_turn_events(state)

    def _prune_turn_events(self, state: _TurnState) -> None:
        if state.thread_id in self._pending_turn_requests or (
            state.thread_id is None and self._pending_turn_requests
        ):
            return
        consumed = min(state.subscribers.values(), default=state.next_event)
        while state.first_event < consumed:
            del state.events[state.first_event]
            state.first_event += 1
        if not state.subscribers and self._turn_states.get(state.id) is state:
            del self._turn_states[state.id]

    def register_turn(self, turn_id: str) -> None:
        """Register the default consumer used by the low-level client API."""
        with self._lock:
            if turn_id not in self._turn_notifications:
                self._turn_notifications[turn_id] = self.subscribe_turn(turn_id)

    def unregister_turn(self, turn_id: str) -> None:
        """Close only the low-level consumer, leaving other handles subscribed."""
        with self._lock:
            if subscription := self._turn_notifications.get(turn_id):
                subscription.close()

    def next_turn_notification(self, turn_id: str) -> Notification:
        """Block until the next event for the default low-level consumer."""
        with self._lock:
            subscription = self._turn_notifications.get(turn_id)
            if subscription is None:
                raise RuntimeError(f"turn {turn_id!r} is not registered for streaming")
        return subscription.next()

    def register_goal(self, thread_id: str) -> _GoalOperationState:
        """Register one thread-scoped logical goal operation before it starts."""
        state = _GoalOperationState(thread_id=thread_id)
        state.activate_turn_routing()
        return self._register_goal(state)

    def reserve_goal(self, thread_id: str) -> _GoalOperationState:
        """Reserve a thread route without accepting physical turns yet."""
        return self._register_goal(_GoalOperationState(thread_id=thread_id))

    def _register_goal(self, state: _GoalOperationState) -> _GoalOperationState:
        with self._lock:
            if state.thread_id in self._goal_operations:
                raise RuntimeError(
                    f"thread {state.thread_id!r} already has an active goal operation"
                )
            self._goal_operations[state.thread_id] = state
        return state

    def unregister_goal(self, state: _GoalOperationState) -> None:
        """Stop routing notifications to a completed logical goal operation."""
        with self._lock:
            if self._goal_operations.get(state.thread_id) is state:
                self._goal_operations.pop(state.thread_id)

    def has_goal(self, thread_id: str) -> bool:
        """Return whether a logical goal operation owns this thread route."""
        with self._lock:
            return thread_id in self._goal_operations

    def route_response(self, msg: dict[str, JsonValue]) -> None:
        """Deliver a JSON-RPC response or error to its request waiter."""

        request_id = msg.get("id")
        with self._lock:
            waiter = self._response_waiters.pop(str(request_id), None)
        if waiter is None:
            return

        if "error" in msg:
            err = msg["error"]
            if isinstance(err, dict):
                waiter.put(
                    map_jsonrpc_error(
                        int(err.get("code", -32000)),
                        str(err.get("message", "unknown")),
                        err.get("data"),
                    )
                )
            else:
                waiter.put(CodexError("Malformed JSON-RPC error response"))
            return

        waiter.put(msg.get("result"))

    def route_notification(self, notification: Notification) -> None:
        """Deliver a notification to a turn queue or the global queue."""

        login_id = self._notification_login_id(notification)
        if login_id is not None:
            with self._lock:
                login_queue = self._login_notifications.get(login_id)
                if login_queue is None:
                    self._pending_login_notifications.setdefault(login_id, deque()).append(
                        notification
                    )
                    return
            login_queue.put(notification)
            return

        turn_id = self._notification_turn_id(notification)
        thread_id = self._notification_thread_id(notification)
        if thread_id is not None:
            with self._lock:
                goal_state = self._goal_operations.get(thread_id)
            if goal_state is not None and (
                turn_id is not None or notification.method.startswith("thread/goal/")
            ):
                if goal_state.observe(notification):
                    if goal_state.is_finished():
                        self.unregister_goal(goal_state)
                    return
        if turn_id is None:
            self._global_notifications.put(notification)
            return

        with self._turn_condition:
            state = self._turn_states.setdefault(turn_id, _TurnState(turn_id, thread_id))
            state.thread_id = thread_id or state.thread_id
            state.events[state.next_event] = notification
            state.next_event += 1
            if notification.method == "turn/completed":
                state.completed = True
            self._prune_turn_events(state)
            self._turn_condition.notify_all()

    def fail_all(self, exc: BaseException) -> None:
        """Wake every blocked waiter when the reader thread exits."""

        with self._lock:
            response_waiters = list(self._response_waiters.values())
            self._response_waiters.clear()
            login_queues = list(self._login_notifications.values())
            self._login_notifications.clear()
            self._pending_login_notifications.clear()
            for thread_id in self._pending_turn_requests:
                self._pending_turn_requests[thread_id] = exc
            for state in list(self._turn_states.values()):
                state.events[state.next_event] = exc
                state.next_event += 1
                state.completed = True
                self._prune_turn_events(state)
            self._turn_condition.notify_all()
            goal_operations = list(self._goal_operations.values())
            self._goal_operations.clear()
        # Put the same transport failure into every queue so no SDK call blocks
        # forever waiting for a response that cannot arrive.
        for waiter in response_waiters:
            waiter.put(exc)
        for login_queue in login_queues:
            login_queue.put(exc)
        for goal_operation in goal_operations:
            goal_operation.fail(exc)
        self._global_notifications.put(exc)

    def _notification_turn_id(self, notification: Notification) -> str | None:
        """Extract routing ids from generated metadata or raw unknown payloads."""
        payload = notification.payload
        if isinstance(payload, UnknownNotification):
            raw_turn_id = payload.params.get("turnId")
            if isinstance(raw_turn_id, str):
                return raw_turn_id
            raw_turn = payload.params.get("turn")
            if isinstance(raw_turn, dict):
                raw_nested_turn_id = raw_turn.get("id")
                if isinstance(raw_nested_turn_id, str):
                    return raw_nested_turn_id
            return None
        return notification_turn_id(payload)

    def _notification_thread_id(self, notification: Notification) -> str | None:
        """Extract thread ids from typed payloads or raw unknown payloads."""
        payload = notification.payload
        if isinstance(payload, UnknownNotification):
            raw_thread_id = payload.params.get("threadId")
            return raw_thread_id if isinstance(raw_thread_id, str) else None
        thread_id = getattr(payload, "thread_id", None)
        return thread_id if isinstance(thread_id, str) else None

    def _notification_login_id(self, notification: Notification) -> str | None:
        """Extract the login attempt id from completion notifications."""
        if notification.method != "account/login/completed":
            return None

        payload = notification.payload
        if isinstance(payload, AccountLoginCompletedNotification):
            return payload.login_id
        if isinstance(payload, UnknownNotification):
            raw_login_id = payload.params.get("loginId")
            if isinstance(raw_login_id, str):
                return raw_login_id
        return None
