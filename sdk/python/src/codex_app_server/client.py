from __future__ import annotations

import json
import os
import subprocess
import threading
import uuid
from collections import deque
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

from .errors import AppServerError, JsonRpcError, TransportClosedError
from .models import Notification
from .typed import ThreadStartResult, TurnStartResult

ApprovalHandler = Callable[[str, dict[str, Any] | None], dict[str, Any]]


@dataclass(slots=True)
class AppServerConfig:
    codex_bin: str = "codex"
    launch_args_override: tuple[str, ...] | None = None
    config_overrides: tuple[str, ...] = ()
    cwd: str | None = None
    env: dict[str, str] | None = None
    client_name: str = "codex_python_sdk"
    client_title: str = "Codex Python SDK"
    client_version: str = "0.1.0"
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
                raise JsonRpcError(err.get("code", -32000), err.get("message", "unknown"), err.get("data"))

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

    def thread_start(self, **params: Any) -> dict[str, Any]:
        return self.request("thread/start", params)

    def thread_resume(self, thread_id: str, **params: Any) -> dict[str, Any]:
        payload = {"threadId": thread_id, **params}
        return self.request("thread/resume", payload)

    def thread_list(self, **params: Any) -> dict[str, Any]:
        return self.request("thread/list", params)

    def thread_read(self, thread_id: str, include_turns: bool = False) -> dict[str, Any]:
        return self.request("thread/read", {"threadId": thread_id, "includeTurns": include_turns})

    def turn_start(self, thread_id: str, input_items: list[dict[str, Any]], **params: Any) -> dict[str, Any]:
        payload = {"threadId": thread_id, "input": input_items, **params}
        return self.request("turn/start", payload)

    def turn_interrupt(self, thread_id: str, turn_id: str) -> dict[str, Any]:
        return self.request("turn/interrupt", {"threadId": thread_id, "turnId": turn_id})

    def model_list(self, include_hidden: bool = False) -> dict[str, Any]:
        return self.request("model/list", {"includeHidden": include_hidden})

    # ---------- Typed convenience wrappers ----------

    def thread_start_typed(self, **params: Any) -> ThreadStartResult:
        return ThreadStartResult.from_dict(self.thread_start(**params))

    def turn_start_typed(
        self, thread_id: str, input_items: list[dict[str, Any]], **params: Any
    ) -> TurnStartResult:
        return TurnStartResult.from_dict(self.turn_start(thread_id, input_items, **params))

    # ---------- Helpers ----------

    def wait_for_turn_completed(self, turn_id: str) -> Notification:
        while True:
            n = self.next_notification()
            if n.method == "turn/completed" and (n.params or {}).get("turn", {}).get("id") == turn_id:
                return n

    def stream_until_methods(self, methods: set[str]) -> list[Notification]:
        out: list[Notification] = []
        while True:
            n = self.next_notification()
            out.append(n)
            if n.method in methods:
                return out

    def run_text_turn(self, thread_id: str, text: str, **params: Any) -> tuple[str, Notification]:
        """Notebook-friendly helper: start a text turn and return (final_text, turn_completed_notification)."""
        turn = self.turn_start(thread_id, input_items=[{"type": "text", "text": text}], **params)
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
