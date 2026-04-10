from __future__ import annotations

import json
import os
import subprocess
import threading
import uuid
from collections import deque
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, Iterable, Iterator, TypeVar

from pydantic import BaseModel

from .errors import AppServerError, TransportClosedError, map_jsonrpc_error
from .generated.notification_registry import NOTIFICATION_MODELS
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
from .models import (
    InitializeResponse,
    JsonObject,
    JsonValue,
    Notification,
    UnknownNotification,
)
from .retry import retry_on_overload

ModelT = TypeVar("ModelT", bound=BaseModel)
ApprovalHandler = Callable[[str, JsonObject | None], JsonObject]
RUNTIME_PKG_NAME = "codex-cli-bin"
GLOBAL_NOTIFICATION_BACKLOG_LIMIT = 512


@dataclass(slots=True)
class _PendingRequest:
    event: threading.Event = field(default_factory=threading.Event)
    result: JsonValue | None = None
    error: BaseException | None = None


class _BufferedNotificationStream:
    def __init__(self, *, maxlen: int | None = None) -> None:
        self._condition = threading.Condition()
        self._items: deque[Notification] = (
            deque(maxlen=maxlen) if maxlen is not None else deque()
        )
        self._closed = False
        self._error: BaseException | None = None

    def push(self, notification: Notification) -> None:
        with self._condition:
            if self._closed:
                return
            self._items.append(notification)
            self._condition.notify_all()

    def pop(self) -> Notification:
        with self._condition:
            while not self._items and not self._closed:
                self._condition.wait()

            if self._items:
                return self._items.popleft()

            if self._error is not None:
                raise self._error

            raise TransportClosedError("notification stream is closed")

    def close(self, error: BaseException | None = None) -> None:
        with self._condition:
            self._closed = True
            self._error = error
            self._condition.notify_all()

    def is_closed(self) -> bool:
        with self._condition:
            return self._closed

    def is_drained(self) -> bool:
        with self._condition:
            return self._closed and not self._items


def _params_dict(
    params: (
        V2ThreadStartParams
        | V2ThreadResumeParams
        | V2ThreadListParams
        | V2ThreadForkParams
        | V2TurnStartParams
        | JsonObject
        | None
    ),
) -> JsonObject:
    if params is None:
        return {}
    if hasattr(params, "model_dump"):
        dumped = params.model_dump(
            by_alias=True,
            exclude_none=True,
            mode="json",
        )
        if not isinstance(dumped, dict):
            raise TypeError("Expected model_dump() to return dict")
        return dumped
    if isinstance(params, dict):
        return params
    raise TypeError(f"Expected generated params model or dict, got {type(params).__name__}")


def _installed_codex_path() -> Path:
    try:
        from codex_cli_bin import bundled_codex_path
    except ImportError as exc:
        raise FileNotFoundError(
            "Unable to locate the pinned Codex runtime. Install the published SDK build "
            f"with its {RUNTIME_PKG_NAME} dependency, or set AppServerConfig.codex_bin "
            "explicitly."
        ) from exc

    return bundled_codex_path()


@dataclass(frozen=True)
class CodexBinResolverOps:
    installed_codex_path: Callable[[], Path]
    path_exists: Callable[[Path], bool]


def _default_codex_bin_resolver_ops() -> CodexBinResolverOps:
    return CodexBinResolverOps(
        installed_codex_path=_installed_codex_path,
        path_exists=lambda path: path.exists(),
    )


def resolve_codex_bin(config: "AppServerConfig", ops: CodexBinResolverOps) -> Path:
    if config.codex_bin is not None:
        codex_bin = Path(config.codex_bin)
        if not ops.path_exists(codex_bin):
            raise FileNotFoundError(
                f"Codex binary not found at {codex_bin}. Set AppServerConfig.codex_bin "
                "to a valid binary path."
            )
        return codex_bin

    return ops.installed_codex_path()


def _resolve_codex_bin(config: "AppServerConfig") -> Path:
    return resolve_codex_bin(config, _default_codex_bin_resolver_ops())


@dataclass(slots=True)
class AppServerConfig:
    codex_bin: str | None = None
    launch_args_override: tuple[str, ...] | None = None
    config_overrides: tuple[str, ...] = ()
    cwd: str | None = None
    env: dict[str, str] | None = None
    client_name: str = "codex_python_sdk"
    client_title: str = "Codex Python SDK"
    client_version: str = "0.2.0"
    experimental_api: bool = True


class AppServerClient:
    """Synchronous typed JSON-RPC client for `codex app-server` over stdio."""

    def __init__(
        self,
        config: AppServerConfig | None = None,
        approval_handler: ApprovalHandler | None = None,
    ) -> None:
        self.config = config or AppServerConfig()
        self._approval_handler = approval_handler or self._default_approval_handler
        self._proc: subprocess.Popen[str] | None = None
        self._write_lock = threading.Lock()
        self._state_lock = threading.Lock()
        self._pending_notifications = _BufferedNotificationStream(
            maxlen=GLOBAL_NOTIFICATION_BACKLOG_LIMIT
        )
        self._pending_requests: dict[str, _PendingRequest] = {}
        self._turn_streams: dict[tuple[str, str], _BufferedNotificationStream] = {}
        self._turn_starting_by_thread_id: set[str] = set()
        self._active_turn_by_thread_id: dict[str, str] = {}
        self._active_turn_consumers: set[tuple[str, str]] = set()
        self._active_turn_stream_count = 0
        self._transport_error: BaseException | None = None
        self._stderr_lines: deque[str] = deque(maxlen=400)
        self._stderr_thread: threading.Thread | None = None
        self._reader_thread: threading.Thread | None = None

    def __enter__(self) -> "AppServerClient":
        self.start()
        return self

    def __exit__(self, _exc_type, _exc, _tb) -> None:
        self.close()

    def start(self) -> None:
        if self._proc is not None:
            return
        self._reset_transport_state()

        if self.config.launch_args_override is not None:
            args = list(self.config.launch_args_override)
        else:
            codex_bin = _resolve_codex_bin(self.config)
            args = [str(codex_bin)]
            for kv in self.config.config_overrides:
                args.extend(["--config", kv])
            args.extend(["app-server", "--listen", "stdio://"])

        env = os.environ.copy()
        if self.config.env:
            env.update(self.config.env)

        self._proc = subprocess.Popen(
            args,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            cwd=self.config.cwd,
            env=env,
            bufsize=1,
        )

        self._start_stderr_drain_thread()
        self._start_reader_thread()

    def close(self) -> None:
        if self._proc is None:
            return
        proc = self._proc
        self._proc = None
        self._finish_transport(TransportClosedError("app-server closed"))

        if proc.stdin:
            proc.stdin.close()
        try:
            proc.terminate()
            proc.wait(timeout=2)
        except Exception:
            proc.kill()

        if self._stderr_thread and self._stderr_thread.is_alive():
            self._stderr_thread.join(timeout=0.5)
        if self._reader_thread and self._reader_thread.is_alive():
            self._reader_thread.join(timeout=0.5)

    def initialize(self) -> InitializeResponse:
        result = self.request(
            "initialize",
            {
                "clientInfo": {
                    "name": self.config.client_name,
                    "title": self.config.client_title,
                    "version": self.config.client_version,
                },
                "capabilities": {
                    "experimentalApi": self.config.experimental_api,
                },
            },
            response_model=InitializeResponse,
        )
        self.notify("initialized", None)
        return result

    def request(
        self,
        method: str,
        params: JsonObject | None,
        *,
        response_model: type[ModelT],
    ) -> ModelT:
        result = self._request_raw(method, params)
        if not isinstance(result, dict):
            raise AppServerError(f"{method} response must be a JSON object")
        return response_model.model_validate(result)

    def _request_raw(self, method: str, params: JsonObject | None = None) -> JsonValue:
        request_id = str(uuid.uuid4())
        waiter = _PendingRequest()
        with self._state_lock:
            if self._transport_error is not None:
                raise self._transport_error
            self._pending_requests[request_id] = waiter

        try:
            self._write_message({"id": request_id, "method": method, "params": params or {}})
        except BaseException:
            with self._state_lock:
                self._pending_requests.pop(request_id, None)
            raise

        waiter.event.wait()
        if waiter.error is not None:
            raise waiter.error
        return waiter.result

    def notify(self, method: str, params: JsonObject | None = None) -> None:
        self._write_message({"method": method, "params": params or {}})

    def next_notification(self) -> Notification:
        with self._state_lock:
            if self._active_turn_stream_count > 0:
                raise RuntimeError(
                    "next_notification() is incompatible with active turn streaming on the same "
                    "client. Consume notifications from TurnHandle.stream()/run() instead."
                )
        return self._pending_notifications.pop()

    def acquire_turn_consumer(self, thread_id: str, turn_id: str) -> None:
        turn_key = (thread_id, turn_id)
        with self._state_lock:
            if turn_key in self._active_turn_consumers:
                raise RuntimeError(
                    f"Turn {turn_id!r} is already being streamed on thread {thread_id!r}."
                )
            self._active_turn_consumers.add(turn_key)
            self._active_turn_stream_count += 1
            self._turn_streams.setdefault(turn_key, _BufferedNotificationStream())

    def release_turn_consumer(self, thread_id: str, turn_id: str) -> None:
        turn_key = (thread_id, turn_id)
        with self._state_lock:
            if turn_key in self._active_turn_consumers:
                self._active_turn_consumers.remove(turn_key)
                self._active_turn_stream_count -= 1
            stream = self._turn_streams.get(turn_key)
            if stream is not None and stream.is_drained():
                self._turn_streams.pop(turn_key, None)

    def next_turn_notification(self, thread_id: str, turn_id: str) -> Notification:
        turn_key = (thread_id, turn_id)
        with self._state_lock:
            stream = self._turn_streams.setdefault(turn_key, _BufferedNotificationStream())
        return stream.pop()

    def assert_can_start_turn(self, thread_id: str) -> None:
        with self._state_lock:
            if thread_id in self._turn_starting_by_thread_id:
                raise RuntimeError(
                    f"Thread {thread_id!r} is already starting a turn on this client."
                )
            active_turn_id = self._active_turn_by_thread_id.get(thread_id)
            if active_turn_id is not None:
                raise RuntimeError(
                    f"Thread {thread_id!r} already has active turn {active_turn_id!r}. "
                    "Use TurnHandle.steer() or TurnHandle.interrupt() instead of starting "
                    "another turn on the same thread."
                )

    def thread_start(self, params: V2ThreadStartParams | JsonObject | None = None) -> ThreadStartResponse:
        return self.request("thread/start", _params_dict(params), response_model=ThreadStartResponse)

    def thread_resume(
        self,
        thread_id: str,
        params: V2ThreadResumeParams | JsonObject | None = None,
    ) -> ThreadResumeResponse:
        payload = {"threadId": thread_id, **_params_dict(params)}
        return self.request("thread/resume", payload, response_model=ThreadResumeResponse)

    def thread_list(self, params: V2ThreadListParams | JsonObject | None = None) -> ThreadListResponse:
        return self.request("thread/list", _params_dict(params), response_model=ThreadListResponse)

    def thread_read(self, thread_id: str, include_turns: bool = False) -> ThreadReadResponse:
        return self.request(
            "thread/read",
            {"threadId": thread_id, "includeTurns": include_turns},
            response_model=ThreadReadResponse,
        )

    def thread_fork(
        self,
        thread_id: str,
        params: V2ThreadForkParams | JsonObject | None = None,
    ) -> ThreadForkResponse:
        payload = {"threadId": thread_id, **_params_dict(params)}
        return self.request("thread/fork", payload, response_model=ThreadForkResponse)

    def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:
        return self.request("thread/archive", {"threadId": thread_id}, response_model=ThreadArchiveResponse)

    def thread_unarchive(self, thread_id: str) -> ThreadUnarchiveResponse:
        return self.request("thread/unarchive", {"threadId": thread_id}, response_model=ThreadUnarchiveResponse)

    def thread_set_name(self, thread_id: str, name: str) -> ThreadSetNameResponse:
        return self.request(
            "thread/name/set",
            {"threadId": thread_id, "name": name},
            response_model=ThreadSetNameResponse,
        )

    def thread_compact(self, thread_id: str) -> ThreadCompactStartResponse:
        return self.request(
            "thread/compact/start",
            {"threadId": thread_id},
            response_model=ThreadCompactStartResponse,
        )

    def turn_start(
        self,
        thread_id: str,
        input_items: list[JsonObject] | JsonObject | str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TurnStartResponse:
        self._begin_turn_start(thread_id)
        payload = {
            **_params_dict(params),
            "threadId": thread_id,
            "input": self._normalize_input_items(input_items),
        }
        try:
            started = self.request("turn/start", payload, response_model=TurnStartResponse)
        except BaseException:
            self._cancel_turn_start(thread_id)
            raise
        self._finish_turn_start(thread_id, started.turn.id)
        return started

    def turn_interrupt(self, thread_id: str, turn_id: str) -> TurnInterruptResponse:
        return self.request(
            "turn/interrupt",
            {"threadId": thread_id, "turnId": turn_id},
            response_model=TurnInterruptResponse,
        )

    def turn_steer(
        self,
        thread_id: str,
        expected_turn_id: str,
        input_items: list[JsonObject] | JsonObject | str,
    ) -> TurnSteerResponse:
        return self.request(
            "turn/steer",
            {
                "threadId": thread_id,
                "expectedTurnId": expected_turn_id,
                "input": self._normalize_input_items(input_items),
            },
            response_model=TurnSteerResponse,
        )

    def model_list(self, include_hidden: bool = False) -> ModelListResponse:
        return self.request(
            "model/list",
            {"includeHidden": include_hidden},
            response_model=ModelListResponse,
        )

    def request_with_retry_on_overload(
        self,
        method: str,
        params: JsonObject | None,
        *,
        response_model: type[ModelT],
        max_attempts: int = 3,
        initial_delay_s: float = 0.25,
        max_delay_s: float = 2.0,
    ) -> ModelT:
        return retry_on_overload(
            lambda: self.request(method, params, response_model=response_model),
            max_attempts=max_attempts,
            initial_delay_s=initial_delay_s,
            max_delay_s=max_delay_s,
        )

    def wait_for_turn_completed(self, turn_id: str) -> TurnCompletedNotification:
        while True:
            notification = self.next_notification()
            if (
                notification.method == "turn/completed"
                and isinstance(notification.payload, TurnCompletedNotification)
                and notification.payload.turn.id == turn_id
            ):
                return notification.payload

    def stream_until_methods(self, methods: Iterable[str] | str) -> list[Notification]:
        target_methods = {methods} if isinstance(methods, str) else set(methods)
        out: list[Notification] = []
        while True:
            notification = self.next_notification()
            out.append(notification)
            if notification.method in target_methods:
                return out

    def stream_text(
        self,
        thread_id: str,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> Iterator[AgentMessageDeltaNotification]:
        started = self.turn_start(thread_id, text, params=params)
        turn_id = started.turn.id
        self.acquire_turn_consumer(thread_id, turn_id)
        try:
            while True:
                notification = self.next_turn_notification(thread_id, turn_id)
                if (
                    notification.method == "item/agentMessage/delta"
                    and isinstance(notification.payload, AgentMessageDeltaNotification)
                    and notification.payload.turn_id == turn_id
                ):
                    yield notification.payload
                    continue
                if (
                    notification.method == "turn/completed"
                    and isinstance(notification.payload, TurnCompletedNotification)
                    and notification.payload.turn.id == turn_id
                ):
                    break
        finally:
            self.release_turn_consumer(thread_id, turn_id)

    def _coerce_notification(self, method: str, params: object) -> Notification:
        params_dict = params if isinstance(params, dict) else {}

        model = NOTIFICATION_MODELS.get(method)
        if model is None:
            return Notification(method=method, payload=UnknownNotification(params=params_dict))

        try:
            payload = model.model_validate(params_dict)
        except Exception:  # noqa: BLE001
            return Notification(method=method, payload=UnknownNotification(params=params_dict))
        return Notification(method=method, payload=payload)

    def _normalize_input_items(
        self,
        input_items: list[JsonObject] | JsonObject | str,
    ) -> list[JsonObject]:
        if isinstance(input_items, str):
            return [{"type": "text", "text": input_items}]
        if isinstance(input_items, dict):
            return [input_items]
        return input_items

    def _default_approval_handler(self, method: str, params: JsonObject | None) -> JsonObject:
        if method == "item/commandExecution/requestApproval":
            return {"decision": "accept"}
        if method == "item/fileChange/requestApproval":
            return {"decision": "accept"}
        return {}

    def _start_stderr_drain_thread(self) -> None:
        if self._proc is None or self._proc.stderr is None:
            return

        def _drain() -> None:
            stderr = self._proc.stderr
            if stderr is None:
                return
            for line in stderr:
                self._stderr_lines.append(line.rstrip("\n"))

        self._stderr_thread = threading.Thread(target=_drain, daemon=True)
        self._stderr_thread.start()

    def _stderr_tail(self, limit: int = 40) -> str:
        return "\n".join(list(self._stderr_lines)[-limit:])

    def _handle_server_request(self, msg: dict[str, JsonValue]) -> JsonObject:
        method = msg["method"]
        params = msg.get("params")
        if not isinstance(method, str):
            return {}
        return self._approval_handler(
            method,
            params if isinstance(params, dict) else None,
        )

    def _write_message(self, payload: JsonObject) -> None:
        if self._proc is None or self._proc.stdin is None:
            raise TransportClosedError("app-server is not running")
        with self._write_lock:
            self._proc.stdin.write(json.dumps(payload) + "\n")
            self._proc.stdin.flush()

    def _read_message(self) -> dict[str, JsonValue]:
        if self._proc is None or self._proc.stdout is None:
            raise TransportClosedError("app-server is not running")

        line = self._proc.stdout.readline()
        if not line:
            raise TransportClosedError(
                f"app-server closed stdout. stderr_tail={self._stderr_tail()[:2000]}"
            )

        try:
            message = json.loads(line)
        except json.JSONDecodeError as exc:
            raise AppServerError(f"Invalid JSON-RPC line: {line!r}") from exc

        if not isinstance(message, dict):
            raise AppServerError(f"Invalid JSON-RPC payload: {message!r}")
        return message

    def _reset_transport_state(self) -> None:
        self._pending_notifications = _BufferedNotificationStream(
            maxlen=GLOBAL_NOTIFICATION_BACKLOG_LIMIT
        )
        self._pending_requests = {}
        self._turn_streams = {}
        self._turn_starting_by_thread_id = set()
        self._active_turn_by_thread_id = {}
        self._active_turn_consumers = set()
        self._active_turn_stream_count = 0
        self._transport_error = None

    def _start_reader_thread(self) -> None:
        def _reader() -> None:
            try:
                while True:
                    msg = self._read_message()
                    if "method" in msg and "id" in msg:
                        self._start_server_request_worker(msg)
                        continue
                    if "method" in msg and "id" not in msg:
                        method = msg["method"]
                        if isinstance(method, str):
                            self._dispatch_notification(
                                self._coerce_notification(method, msg.get("params"))
                            )
                        continue
                    self._handle_response_message(msg)
            except BaseException as exc:  # noqa: BLE001
                self._finish_transport(exc)

        self._reader_thread = threading.Thread(target=_reader, daemon=True)
        self._reader_thread.start()

    def _start_server_request_worker(self, msg: dict[str, JsonValue]) -> None:
        def _resolve() -> None:
            try:
                response = self._handle_server_request(msg)
                self._write_message({"id": msg["id"], "result": response})
            except BaseException:
                return

        threading.Thread(target=_resolve, daemon=True).start()

    def _handle_response_message(self, msg: dict[str, JsonValue]) -> None:
        request_id = msg.get("id")
        if not isinstance(request_id, str):
            return

        with self._state_lock:
            waiter = self._pending_requests.pop(request_id, None)

        if waiter is None:
            return

        if "error" in msg:
            err = msg["error"]
            if isinstance(err, dict):
                waiter.error = map_jsonrpc_error(
                    int(err.get("code", -32000)),
                    str(err.get("message", "unknown")),
                    err.get("data"),
                )
            else:
                waiter.error = AppServerError("Malformed JSON-RPC error response")
        else:
            waiter.result = msg.get("result")
        waiter.event.set()

    def _dispatch_notification(self, notification: Notification) -> None:
        self._pending_notifications.push(notification)

        turn_key = self._turn_key_for_notification(notification)
        if turn_key is None:
            return

        thread_id, turn_id = turn_key
        close_stream = False
        with self._state_lock:
            stream = self._turn_streams.setdefault(turn_key, _BufferedNotificationStream())
            if notification.method == "turn/started":
                self._turn_starting_by_thread_id.discard(thread_id)
                self._active_turn_by_thread_id[thread_id] = turn_id
            elif notification.method == "turn/completed":
                self._turn_starting_by_thread_id.discard(thread_id)
                if self._active_turn_by_thread_id.get(thread_id) == turn_id:
                    self._active_turn_by_thread_id.pop(thread_id, None)
                close_stream = True

        stream.push(notification)
        if close_stream:
            stream.close()

    def _turn_key_for_notification(self, notification: Notification) -> tuple[str, str] | None:
        payload = notification.payload
        thread_id = getattr(payload, "thread_id", None)
        turn_id = getattr(payload, "turn_id", None)
        if isinstance(thread_id, str) and isinstance(turn_id, str):
            return thread_id, turn_id

        turn = getattr(payload, "turn", None)
        nested_turn_id = getattr(turn, "id", None)
        if isinstance(thread_id, str) and isinstance(nested_turn_id, str):
            return thread_id, nested_turn_id

        return None

    def _begin_turn_start(self, thread_id: str) -> None:
        with self._state_lock:
            active_turn_id = self._active_turn_by_thread_id.get(thread_id)
            if active_turn_id is not None:
                raise RuntimeError(
                    f"Thread {thread_id!r} already has active turn {active_turn_id!r}. "
                    "Use TurnHandle.steer() or TurnHandle.interrupt() instead of starting "
                    "another turn on the same thread."
                )
            if thread_id in self._turn_starting_by_thread_id:
                raise RuntimeError(
                    f"Thread {thread_id!r} is already starting a turn on this client."
                )
            self._turn_starting_by_thread_id.add(thread_id)

    def _cancel_turn_start(self, thread_id: str) -> None:
        with self._state_lock:
            self._turn_starting_by_thread_id.discard(thread_id)

    def _finish_turn_start(self, thread_id: str, turn_id: str) -> None:
        turn_key = (thread_id, turn_id)
        with self._state_lock:
            self._turn_starting_by_thread_id.discard(thread_id)
            stream = self._turn_streams.setdefault(turn_key, _BufferedNotificationStream())
            if not stream.is_closed():
                self._active_turn_by_thread_id[thread_id] = turn_id

    def _finish_transport(self, error: BaseException) -> None:
        with self._state_lock:
            if self._transport_error is not None:
                return
            self._transport_error = error
            pending_requests = list(self._pending_requests.values())
            self._pending_requests.clear()
            turn_streams = list(self._turn_streams.values())
            self._turn_streams.clear()
            self._turn_starting_by_thread_id.clear()
            self._active_turn_by_thread_id.clear()
            self._active_turn_consumers.clear()
            self._active_turn_stream_count = 0

        for waiter in pending_requests:
            waiter.error = error
            waiter.event.set()

        self._pending_notifications.close(error)
        for stream in turn_streams:
            stream.close(error)


def default_codex_home() -> str:
    return str(Path.home() / ".codex")
