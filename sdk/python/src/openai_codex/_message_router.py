from __future__ import annotations

import queue
import threading
from collections import deque

from .errors import AppServerError, map_jsonrpc_error
from .generated.notification_registry import notification_turn_id
from .models import JsonValue, Notification, UnknownNotification

ResponseQueueItem = JsonValue | BaseException
NotificationQueueItem = Notification | BaseException


class MessageRouter:
    """Route reader-thread messages to the SDK operation waiting for them.

    The app-server stdio transport is a single ordered stream, so only the
    reader thread should consume stdout. This router keeps the rest of the SDK
    from competing for that stream by giving each in-flight JSON-RPC request
    and active turn stream its own queue.
    """

    def __init__(self) -> None:
        """Create empty response, turn, and global notification queues."""
        self._lock = threading.Lock()
        self._response_waiters: dict[str, queue.Queue[ResponseQueueItem]] = {}
        self._turn_notifications: dict[str, queue.Queue[NotificationQueueItem]] = {}
        self._pending_turn_notifications: dict[str, deque[Notification]] = {}
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

    def register_turn(self, turn_id: str) -> None:
        """Register a queue for a turn stream and replay early events."""

        turn_queue: queue.Queue[NotificationQueueItem] = queue.Queue()
        with self._lock:
            if turn_id in self._turn_notifications:
                return
            # A turn can emit events immediately after turn/start, before the
            # caller receives the TurnHandle and starts streaming.
            pending = self._pending_turn_notifications.pop(turn_id, deque())
            self._turn_notifications[turn_id] = turn_queue
        for notification in pending:
            turn_queue.put(notification)

    def unregister_turn(self, turn_id: str) -> None:
        """Stop routing future turn events to the stream queue."""

        with self._lock:
            self._turn_notifications.pop(turn_id, None)

    def next_turn_notification(self, turn_id: str) -> Notification:
        """Block until the next notification for a registered turn."""

        with self._lock:
            turn_queue = self._turn_notifications.get(turn_id)
        if turn_queue is None:
            raise RuntimeError(f"turn {turn_id!r} is not registered for streaming")
        item = turn_queue.get()
        if isinstance(item, BaseException):
            raise item
        return item

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
                waiter.put(AppServerError("Malformed JSON-RPC error response"))
            return

        waiter.put(msg.get("result"))

    def route_notification(self, notification: Notification) -> None:
        """Deliver a notification to a turn queue or the global queue."""

        turn_id = self._notification_turn_id(notification)
        if turn_id is None:
            self._global_notifications.put(notification)
            return

        with self._lock:
            turn_queue = self._turn_notifications.get(turn_id)
            if turn_queue is None:
                if notification.method == "turn/completed":
                    self._pending_turn_notifications.pop(turn_id, None)
                    return
                self._pending_turn_notifications.setdefault(turn_id, deque()).append(
                    notification
                )
                return
        turn_queue.put(notification)

    def fail_all(self, exc: BaseException) -> None:
        """Wake every blocked waiter when the reader thread exits."""

        with self._lock:
            response_waiters = list(self._response_waiters.values())
            self._response_waiters.clear()
            turn_queues = list(self._turn_notifications.values())
            self._pending_turn_notifications.clear()
        # Put the same transport failure into every queue so no SDK call blocks
        # forever waiting for a response that cannot arrive.
        for waiter in response_waiters:
            waiter.put(exc)
        for turn_queue in turn_queues:
            turn_queue.put(exc)
        self._global_notifications.put(exc)

    def _notification_turn_id(self, notification: Notification) -> str | None:
        """Extract routing ids from known generated payloads or raw unknown payloads."""
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
