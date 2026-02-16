from __future__ import annotations

import json
import os
import subprocess
import threading
import uuid
from collections import deque
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Iterable

from .errors import AppServerError, JsonRpcError, TransportClosedError, map_jsonrpc_error
from .models import Notification
from .typed import (
    AgentMessageDeltaEvent,
    EmptyResult,
    ErrorEvent,
    ItemLifecycleEvent,
    ModelListResult,
    ThreadForkResult,
    ThreadListResult,
    ThreadNameUpdatedEvent,
    ThreadReadResult,
    ThreadResumeResult,
    ThreadStartResult,
    ThreadStartedEvent,
    ThreadTokenUsageUpdatedEvent,
    TurnCompletedEvent,
    TurnStartResult,
    TurnSteerResult,
    TurnStartedEvent,
)
from .retry import retry_on_overload
from .conversation import Conversation
from .schema_types import (
    AgentMessageDeltaNotificationPayload as SchemaAgentMessageDeltaNotificationPayload,
    ErrorNotificationPayload as SchemaErrorNotificationPayload,
    ModelListResponse as SchemaModelListResponse,
    ItemCompletedNotificationPayload as SchemaItemCompletedNotificationPayload,
    ItemStartedNotificationPayload as SchemaItemStartedNotificationPayload,
    ThreadArchiveResponse as SchemaThreadArchiveResponse,
    ThreadForkResponse as SchemaThreadForkResponse,
    ThreadListResponse as SchemaThreadListResponse,
    ThreadNameUpdatedNotificationPayload as SchemaThreadNameUpdatedNotificationPayload,
    ThreadReadResponse as SchemaThreadReadResponse,
    ThreadResumeResponse as SchemaThreadResumeResponse,
    ThreadStartResponse as SchemaThreadStartResponse,
    ThreadStartedNotificationPayload as SchemaThreadStartedNotificationPayload,
    ThreadTokenUsageUpdatedNotificationPayload as SchemaThreadTokenUsageUpdatedNotificationPayload,
    ThreadUnarchiveResponse as SchemaThreadUnarchiveResponse,
    ThreadSetNameResponse as SchemaThreadSetNameResponse,
    TurnCompletedNotificationPayload as SchemaTurnCompletedNotificationPayload,
    TurnStartResponse as SchemaTurnStartResponse,
    TurnSteerResponse as SchemaTurnSteerResponse,
    TurnStartedNotificationPayload as SchemaTurnStartedNotificationPayload,
)
from .protocol_types import (
    ThreadListResponse,
    ThreadReadResponse,
    ThreadResumeResponse,
    ThreadStartResponse,
    TurnStartResponse,
)

ApprovalHandler = Callable[[str, dict[str, Any] | None], dict[str, Any]]

_TYPED_NOTIFICATION_PARSERS = {
    "turn/completed": TurnCompletedEvent,
    "turn/started": TurnStartedEvent,
    "thread/started": ThreadStartedEvent,
    "item/agentMessage/delta": AgentMessageDeltaEvent,
    "item/started": ItemLifecycleEvent,
    "item/completed": ItemLifecycleEvent,
    "thread/nameUpdated": ThreadNameUpdatedEvent,
    "thread/tokenUsageUpdated": ThreadTokenUsageUpdatedEvent,
    "error": ErrorEvent,
}

_SCHEMA_NOTIFICATION_PARSERS = {
    "turn/completed": SchemaTurnCompletedNotificationPayload,
    "turn/started": SchemaTurnStartedNotificationPayload,
    "thread/started": SchemaThreadStartedNotificationPayload,
    "item/agentMessage/delta": SchemaAgentMessageDeltaNotificationPayload,
    "item/started": SchemaItemStartedNotificationPayload,
    "item/completed": SchemaItemCompletedNotificationPayload,
    "thread/nameUpdated": SchemaThreadNameUpdatedNotificationPayload,
    "thread/tokenUsageUpdated": SchemaThreadTokenUsageUpdatedNotificationPayload,
    "error": SchemaErrorNotificationPayload,
}


@dataclass(slots=True)
class AppServerConfig:
    codex_bin: str = "codex"
    launch_args_override: tuple[str, ...] | None = None
    config_overrides: tuple[str, ...] = ()
    cwd: str | None = None
    env: dict[str, str] | None = None
    client_name: str = "codex_python_sdk"
    client_title: str = "Codex Python SDK"
    client_version: str = "0.2.0"
    experimental_api: bool = True


class AppServerClient:
    """Synchronous JSON-RPC client for `codex app-server` over stdio."""

    def __init__(
        self,
        config: AppServerConfig | None = None,
        approval_handler: ApprovalHandler | None = None,
    ) -> None:
        self.config = config or AppServerConfig()
        self._approval_handler = approval_handler or self._default_approval_handler
        self._proc: subprocess.Popen[str] | None = None
        self._lock = threading.Lock()
        self._pending_notifications: deque[Notification] = deque()
        self._stderr_lines: deque[str] = deque(maxlen=400)
        self._stderr_thread: threading.Thread | None = None

    def __enter__(self) -> "AppServerClient":
        self.start()
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def start(self) -> None:
        if self._proc is not None:
            return

        if self.config.launch_args_override is not None:
            args = list(self.config.launch_args_override)
        else:
            args = [self.config.codex_bin]
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
            cwd=self.config.cwd,
            env=env,
            bufsize=1,
        )

        self._start_stderr_drain_thread()

    def close(self) -> None:
        if self._proc is None:
            return
        proc = self._proc
        self._proc = None

        if proc.stdin:
            proc.stdin.close()
        try:
            proc.terminate()
            proc.wait(timeout=2)
        except Exception:
            proc.kill()

        if self._stderr_thread and self._stderr_thread.is_alive():
            self._stderr_thread.join(timeout=0.5)

    # ---------- Core JSON-RPC ----------

    def initialize(self) -> dict[str, Any]:
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
        )
        self.notify("initialized", None)
        return result

    def request(self, method: str, params: dict[str, Any] | None = None) -> Any:
        request_id = str(uuid.uuid4())
        self._write_message({"id": request_id, "method": method, "params": params or {}})

        while True:
            msg = self._read_message()

            if "method" in msg and "id" in msg:
                response = self._handle_server_request(msg)
                self._write_message({"id": msg["id"], "result": response})
                continue

            if "method" in msg and "id" not in msg:
                self._pending_notifications.append(
                    Notification(method=msg["method"], params=msg.get("params"))
                )
                continue

            if msg.get("id") != request_id:
                continue

            if "error" in msg:
                err = msg["error"]
                raise map_jsonrpc_error(
                    int(err.get("code", -32000)),
                    str(err.get("message", "unknown")),
                    err.get("data"),
                )

            return msg.get("result")

    def notify(self, method: str, params: dict[str, Any] | None = None) -> None:
        self._write_message({"method": method, "params": params or {}})

    def next_notification(self) -> Notification:
        if self._pending_notifications:
            return self._pending_notifications.popleft()

        while True:
            msg = self._read_message()
            if "method" in msg and "id" in msg:
                response = self._handle_server_request(msg)
                self._write_message({"id": msg["id"], "result": response})
                continue
            if "method" in msg and "id" not in msg:
                return Notification(method=msg["method"], params=msg.get("params"))

    # ---------- High-level v2 API ----------

    def thread_start(self, **params: Any) -> ThreadStartResponse:
        return self.request("thread/start", params)

    def thread_resume(self, thread_id: str, **params: Any) -> ThreadResumeResponse:
        payload = {"threadId": thread_id, **params}
        return self.request("thread/resume", payload)

    def thread_list(self, **params: Any) -> ThreadListResponse:
        return self.request("thread/list", params)

    def thread_read(self, thread_id: str, include_turns: bool = False) -> ThreadReadResponse:
        return self.request("thread/read", {"threadId": thread_id, "includeTurns": include_turns})

    def thread_fork(self, thread_id: str, **params: Any) -> dict[str, Any]:
        return self.request("thread/fork", {"threadId": thread_id, **params})

    def thread_archive(self, thread_id: str) -> dict[str, Any]:
        return self.request("thread/archive", {"threadId": thread_id})

    def thread_unarchive(self, thread_id: str) -> dict[str, Any]:
        return self.request("thread/unarchive", {"threadId": thread_id})

    def thread_set_name(self, thread_id: str, name: str) -> dict[str, Any]:
        return self.request("thread/setName", {"threadId": thread_id, "name": name})

    def turn_start(
        self,
        thread_id: str,
        input_items: list[dict[str, Any]] | dict[str, Any] | str,
        **params: Any,
    ) -> TurnStartResponse:
        payload = {"threadId": thread_id, "input": self._normalize_input_items(input_items), **params}
        return self.request("turn/start", payload)

    def turn_text(self, thread_id: str, text: str, **params: Any) -> TurnStartResponse:
        """Convenience helper for the common text-only turn case."""
        return self.turn_start(thread_id, text, **params)

    def turn_interrupt(self, thread_id: str, turn_id: str) -> dict[str, Any]:
        return self.request("turn/interrupt", {"threadId": thread_id, "turnId": turn_id})

    def turn_steer(
        self,
        thread_id: str,
        expected_turn_id: str,
        input_items: list[dict[str, Any]] | dict[str, Any] | str,
    ) -> dict[str, Any]:
        return self.request(
            "turn/steer",
            {
                "threadId": thread_id,
                "expectedTurnId": expected_turn_id,
                "input": self._normalize_input_items(input_items),
            },
        )

    def model_list(self, include_hidden: bool = False) -> dict[str, Any]:
        return self.request("model/list", {"includeHidden": include_hidden})

    def conversation(self, thread_id: str) -> Conversation:
        return Conversation(client=self, thread_id=thread_id)

    def conversation_start(self, *, model: str | None = None, **params: Any) -> Conversation:
        payload = dict(params)
        if model is not None:
            payload["model"] = model
        started = self.thread_start(**payload)
        return Conversation(client=self, thread_id=started["thread"]["id"])

    # ---------- Typed convenience wrappers ----------

    def thread_start_typed(self, **params: Any) -> ThreadStartResult:
        return ThreadStartResult.from_dict(self.thread_start(**params))

    def thread_resume_typed(self, thread_id: str, **params: Any) -> ThreadResumeResult:
        return ThreadResumeResult.from_dict(self.thread_resume(thread_id, **params))

    def thread_read_typed(self, thread_id: str, include_turns: bool = False) -> ThreadReadResult:
        return ThreadReadResult.from_dict(self.thread_read(thread_id, include_turns=include_turns))

    def thread_fork_typed(self, thread_id: str, **params: Any) -> ThreadForkResult:
        return ThreadForkResult.from_dict(self.thread_fork(thread_id, **params))

    def thread_archive_typed(self, thread_id: str) -> EmptyResult:
        return EmptyResult.from_dict(self.thread_archive(thread_id))

    def thread_unarchive_typed(self, thread_id: str) -> EmptyResult:
        return EmptyResult.from_dict(self.thread_unarchive(thread_id))

    def thread_set_name_typed(self, thread_id: str, name: str) -> EmptyResult:
        return EmptyResult.from_dict(self.thread_set_name(thread_id, name))

    def thread_list_typed(self, **params: Any) -> ThreadListResult:
        return ThreadListResult.from_dict(self.thread_list(**params))

    def model_list_typed(self, include_hidden: bool = False) -> ModelListResult:
        return ModelListResult.from_dict(self.model_list(include_hidden=include_hidden))

    def turn_start_typed(
        self,
        thread_id: str,
        input_items: list[dict[str, Any]] | dict[str, Any] | str,
        **params: Any,
    ) -> TurnStartResult:
        return TurnStartResult.from_dict(self.turn_start(thread_id, input_items, **params))

    def turn_text_typed(self, thread_id: str, text: str, **params: Any) -> TurnStartResult:
        return TurnStartResult.from_dict(self.turn_text(thread_id, text, **params))

    def turn_steer_typed(
        self,
        thread_id: str,
        expected_turn_id: str,
        input_items: list[dict[str, Any]] | dict[str, Any] | str,
    ) -> TurnSteerResult:
        return TurnSteerResult.from_dict(self.turn_steer(thread_id, expected_turn_id, input_items))

    def thread_start_schema(self, **params: Any) -> SchemaThreadStartResponse:
        return SchemaThreadStartResponse.from_dict(self.thread_start(**params))

    def thread_resume_schema(self, thread_id: str, **params: Any) -> SchemaThreadResumeResponse:
        return SchemaThreadResumeResponse.from_dict(self.thread_resume(thread_id, **params))

    def thread_read_schema(self, thread_id: str, include_turns: bool = False) -> SchemaThreadReadResponse:
        return SchemaThreadReadResponse.from_dict(self.thread_read(thread_id, include_turns=include_turns))

    def thread_list_schema(self, **params: Any) -> SchemaThreadListResponse:
        return SchemaThreadListResponse.from_dict(self.thread_list(**params))

    def thread_fork_schema(self, thread_id: str, **params: Any) -> SchemaThreadForkResponse:
        return SchemaThreadForkResponse.from_dict(self.thread_fork(thread_id, **params))

    def thread_archive_schema(self, thread_id: str) -> SchemaThreadArchiveResponse:
        return SchemaThreadArchiveResponse.from_dict(self.thread_archive(thread_id))

    def thread_unarchive_schema(self, thread_id: str) -> SchemaThreadUnarchiveResponse:
        return SchemaThreadUnarchiveResponse.from_dict(self.thread_unarchive(thread_id))

    def thread_set_name_schema(self, thread_id: str, name: str) -> SchemaThreadSetNameResponse:
        return SchemaThreadSetNameResponse.from_dict(self.thread_set_name(thread_id, name))

    def model_list_schema(self, include_hidden: bool = False) -> SchemaModelListResponse:
        return SchemaModelListResponse.from_dict(self.model_list(include_hidden=include_hidden))

    def turn_start_schema(
        self,
        thread_id: str,
        input_items: list[dict[str, Any]] | dict[str, Any] | str,
        **params: Any,
    ) -> SchemaTurnStartResponse:
        return SchemaTurnStartResponse.from_dict(self.turn_start(thread_id, input_items, **params))

    def turn_text_schema(self, thread_id: str, text: str, **params: Any) -> SchemaTurnStartResponse:
        return self.turn_start_schema(thread_id, text, **params)

    def turn_steer_schema(
        self,
        thread_id: str,
        expected_turn_id: str,
        input_items: list[dict[str, Any]] | dict[str, Any] | str,
    ) -> SchemaTurnSteerResponse:
        return SchemaTurnSteerResponse.from_dict(self.turn_steer(thread_id, expected_turn_id, input_items))

    def parse_notification_typed(
        self, notification: Notification
    ) -> (
        TurnCompletedEvent
        | TurnStartedEvent
        | ThreadStartedEvent
        | AgentMessageDeltaEvent
        | ItemLifecycleEvent
        | ThreadNameUpdatedEvent
        | ThreadTokenUsageUpdatedEvent
        | ErrorEvent
        | None
    ):
        return self._parse_notification_with(notification, _TYPED_NOTIFICATION_PARSERS)

    def parse_notification_schema(
        self, notification: Notification
    ) -> (
        SchemaTurnCompletedNotificationPayload
        | SchemaTurnStartedNotificationPayload
        | SchemaThreadStartedNotificationPayload
        | SchemaAgentMessageDeltaNotificationPayload
        | SchemaItemStartedNotificationPayload
        | SchemaItemCompletedNotificationPayload
        | SchemaThreadNameUpdatedNotificationPayload
        | SchemaThreadTokenUsageUpdatedNotificationPayload
        | SchemaErrorNotificationPayload
        | None
    ):
        return self._parse_notification_with(notification, _SCHEMA_NOTIFICATION_PARSERS)

    def request_with_retry_on_overload(
        self,
        method: str,
        params: dict[str, Any] | None = None,
        *,
        max_attempts: int = 3,
        initial_delay_s: float = 0.25,
        max_delay_s: float = 2.0,
    ) -> Any:
        return retry_on_overload(
            lambda: self.request(method, params),
            max_attempts=max_attempts,
            initial_delay_s=initial_delay_s,
            max_delay_s=max_delay_s,
        )

    # ---------- Helpers ----------

    def wait_for_turn_completed(self, turn_id: str) -> Notification:
        while True:
            n = self.next_notification()
            if n.method == "turn/completed" and (n.params or {}).get("turn", {}).get("id") == turn_id:
                return n

    def stream_until_methods(self, methods: Iterable[str] | str) -> list[Notification]:
        target_methods = {methods} if isinstance(methods, str) else set(methods)
        out: list[Notification] = []
        while True:
            n = self.next_notification()
            out.append(n)
            if n.method in target_methods:
                return out

    def run_text_turn(self, thread_id: str, text: str, **params: Any) -> tuple[str, Notification]:
        """Notebook-friendly helper: start a text turn and return (final_text, turn_completed_notification)."""
        turn = self.turn_text(thread_id, text, **params)
        turn_id = turn["turn"]["id"]

        chunks: list[str] = []
        completed: Notification | None = None
        while True:
            n = self.next_notification()
            if n.method == "item/agentMessage/delta":
                chunks.append((n.params or {}).get("delta", ""))
            if n.method == "turn/completed" and (n.params or {}).get("turn", {}).get("id") == turn_id:
                completed = n
                break

        assert completed is not None
        return "".join(chunks), completed

    def ask(self, text: str, *, model: str | None = None, thread_id: str | None = None) -> tuple[str, str]:
        """High-level helper for notebooks.

        Returns `(thread_id, assistant_text)`.
        - If `thread_id` is omitted, starts a fresh thread.
        - If provided, appends a turn to existing thread.
        """
        if thread_id is None:
            started = self.thread_start(**({"model": model} if model else {}))
            thread_id = started["thread"]["id"]
        assistant_text, _ = self.run_text_turn(thread_id, text)
        return thread_id, assistant_text

    # ---------- Internals ----------

    def _parse_notification_with(self, notification: Notification, parsers: dict[str, Any]) -> Any | None:
        parser = parsers.get(notification.method)
        if parser is None:
            return None
        return parser.from_dict(notification.params or {})

    def _normalize_input_items(
        self, input_items: list[dict[str, Any]] | dict[str, Any] | str
    ) -> list[dict[str, Any]]:
        if isinstance(input_items, str):
            return [{"type": "text", "text": input_items}]
        if isinstance(input_items, dict):
            return [input_items]
        return input_items

    def _default_approval_handler(self, method: str, params: dict[str, Any] | None) -> dict[str, Any]:
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

    def _handle_server_request(self, msg: dict[str, Any]) -> dict[str, Any]:
        method = msg["method"]
        params = msg.get("params")
        return self._approval_handler(method, params)

    def _write_message(self, payload: dict[str, Any]) -> None:
        if self._proc is None or self._proc.stdin is None:
            raise TransportClosedError("app-server is not running")
        with self._lock:
            self._proc.stdin.write(json.dumps(payload) + "\n")
            self._proc.stdin.flush()

    def _read_message(self) -> dict[str, Any]:
        if self._proc is None or self._proc.stdout is None:
            raise TransportClosedError("app-server is not running")

        line = self._proc.stdout.readline()
        if not line:
            raise TransportClosedError(
                f"app-server closed stdout. stderr_tail={self._stderr_tail()[:2000]}"
            )

        try:
            return json.loads(line)
        except json.JSONDecodeError as exc:
            raise AppServerError(f"Invalid JSON-RPC line: {line!r}") from exc


def default_codex_home() -> str:
    return str(Path.home() / ".codex")
