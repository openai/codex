#!/usr/bin/env python3
"""Show a macOS approval dialog via PyObjC/AppKit with JSON input and output.

The helper wraps the working NSAlert + accessory-view pattern we prototyped:
- frontmost activation
- optional thread / reason / permission-rule labels
- optional monospaced code box
- custom buttons with single-key equivalents
- JSON result on stdout

Input is read from stdin by default, or from --input. Example:

{
  "kind": "exec",
  "thread": "Robie [explorer]",
  "reason": "run the targeted test suite before finalizing changes.",
  "permission_rule": "write `/tmp`, `/Users/ebrevdo/code/codex`",
  "code": "python -m pytest tests/test_example.py\\n--maxfail=1\\n-q"
}
"""

from __future__ import annotations

import argparse
import json
import os
import select
import shlex
import sys
import tempfile
import textwrap
import time
import warnings
from datetime import datetime
from pathlib import Path
from typing import Any


DEFAULT_WIDTH = 620
DEFAULT_CODE_HEIGHT = 180
DEFAULT_SHORTCUT_ENABLE_DELAY_MS = 1_750
NATIVE_BUTTON_MIN_HEIGHT = 36
NATIVE_BUTTON_VERTICAL_PADDING = 18
BUTTON_WRAP_WIDTH = 72
TITLE_WRAP_WIDTH = 52
BODY_WRAP_WIDTH = 72
BUTTON_VERTICAL_GAP = 4
DEBUG_ENV = "CODEX_APPROVAL_DIALOG_DEBUG"
DEBUG_DIR_ENV = "CODEX_APPROVAL_DIALOG_DEBUG_DIR"
REQUESTER_PID_ENV = "CODEX_APPROVAL_REQUESTER_PID"
STDIN_TIMEOUT_ENV = "CODEX_APPROVAL_DIALOG_STDIN_TIMEOUT_MS"
DEFAULT_STDIN_TIMEOUT_MS = 100


def _render_command(command: list[str]) -> str:
    try:
        return shlex.join(command)
    except Exception:
        return " ".join(command)


def _format_permission_rule(additional_permissions: dict[str, Any] | None) -> str | None:
    if not additional_permissions:
        return None
    parts: list[str] = []
    file_system = additional_permissions.get("file_system")
    if isinstance(file_system, dict):
        read = file_system.get("read")
        if isinstance(read, list) and read:
            parts.append("read " + ", ".join(f"`{item}`" for item in read))
        write = file_system.get("write")
        if isinstance(write, list) and write:
            parts.append("write " + ", ".join(f"`{item}`" for item in write))
    return "; ".join(parts) if parts else None


def _decision_id(decision: Any) -> str:
    if isinstance(decision, str):
        return decision
    if isinstance(decision, dict) and len(decision) == 1:
        return next(iter(decision))
    raise ValueError(f"unsupported review decision shape: {decision!r}")


def _execpolicy_command_prefix(amendment: Any) -> list[str]:
    if isinstance(amendment, list) and all(isinstance(item, str) for item in amendment):
        return amendment
    if isinstance(amendment, dict):
        command = amendment.get("command")
        if isinstance(command, list) and all(isinstance(item, str) for item in command):
            return command
    raise ValueError(f"unsupported execpolicy amendment shape: {amendment!r}")


def _option_for_exec_decision(
    decision: Any,
    *,
    network_approval_context: dict[str, Any] | None,
    additional_permissions: dict[str, Any] | None,
) -> dict[str, Any]:
    decision_id = _decision_id(decision)
    option: dict[str, Any]
    if decision_id == "approved":
        option = {
            "id": "approved",
            "label": "Yes, just this once" if network_approval_context else "Yes, proceed",
            "key": "y",
            "default": True,
        }
    elif decision_id == "approved_execpolicy_amendment":
        amendment = decision["approved_execpolicy_amendment"]
        prefix = _render_command(
            _execpolicy_command_prefix(amendment["proposed_execpolicy_amendment"])
        )
        option = {
            "id": "approved_execpolicy_amendment",
            "label": f"Yes, and don't ask again for commands that start with `{prefix}`",
            "key": "p",
        }
    elif decision_id == "approved_for_session":
        if network_approval_context:
            label = "Yes, and allow this host for this conversation"
        elif additional_permissions:
            label = "Yes, and allow these permissions for this session"
        else:
            label = "Yes, and don't ask again for this command in this session"
        option = {"id": "approved_for_session", "label": label, "key": "a"}
    elif decision_id == "network_policy_amendment":
        amendment = decision["network_policy_amendment"]
        action = amendment["action"]
        if action == "allow":
            option = {
                "id": "network_policy_amendment_allow",
                "label": "Yes, and allow this host in the future",
                "key": "p",
            }
        elif action == "deny":
            option = {
                "id": "network_policy_amendment_deny",
                "label": "No, and block this host in the future",
                "key": "d",
            }
        else:
            raise ValueError(f"unsupported network policy action: {action!r}")
    elif decision_id == "denied":
        option = {
            "id": "denied",
            "label": "No, continue without running it",
            "key": "d",
        }
    elif decision_id == "abort":
        option = {
            "id": "abort",
            "label": "No, and tell Codex what to do differently",
            "key": "n",
            "cancel": True,
        }
    else:
        raise ValueError(f"unsupported exec decision: {decision!r}")

    option["decision"] = decision
    return option


def _default_exec_decisions(raw: dict[str, Any]) -> list[Any]:
    if raw.get("available_decisions") is not None:
        return raw["available_decisions"]
    network_approval_context = raw.get("network_approval_context")
    if network_approval_context is not None:
        decisions: list[Any] = ["approved", "approved_for_session"]
        amendments = raw.get("proposed_network_policy_amendments") or []
        allow_amendment = next(
            (item for item in amendments if isinstance(item, dict) and item.get("action") == "allow"),
            None,
        )
        if allow_amendment is not None:
            decisions.append({"network_policy_amendment": allow_amendment})
        decisions.append("abort")
        return decisions
    if raw.get("additional_permissions") is not None:
        return ["approved", "abort"]
    decisions = ["approved"]
    amendment = raw.get("proposed_execpolicy_amendment")
    if amendment is not None:
        decisions.append({"approved_execpolicy_amendment": {"proposed_execpolicy_amendment": amendment}})
    decisions.append("abort")
    return decisions


def _summarize_patch_changes(changes: dict[str, Any]) -> str:
    lines = []
    for path, change in changes.items():
        if not isinstance(change, dict):
            lines.append(f"? {path}")
            continue
        change_type = change.get("type", "?")
        symbol = {
            "add": "A",
            "delete": "D",
            "update": "M",
        }.get(change_type, "?")
        lines.append(f"{symbol} {path}")
    return "\n".join(lines)


def _normalize_exec_request(raw: dict[str, Any]) -> dict[str, Any]:
    network_approval_context = raw.get("network_approval_context")
    additional_permissions = raw.get("additional_permissions")
    payload: dict[str, Any] = {
        "kind": "network" if network_approval_context else "exec",
        "window_title": "Approval Request",
        "title": _default_title(
            {
                "kind": "network" if network_approval_context else "exec",
                "host": (network_approval_context or {}).get("host"),
            }
        ),
        "message": "Codex needs your approval before continuing.",
        "reason": raw.get("reason"),
        "thread_id": raw.get("thread_id"),
        "thread_label": raw.get("thread_label"),
        "thread": raw.get("thread_label"),
        "is_current_thread": raw.get("is_current_thread"),
        "current_thread_id": raw.get("current_thread_id"),
        "permission_rule": _format_permission_rule(additional_permissions),
        "host": (network_approval_context or {}).get("host"),
        "code": None if network_approval_context else _render_command(raw["command"]),
        "show_shortcuts_hint": False,
        "code_selectable": False,
        "width": DEFAULT_WIDTH,
        "code_height": DEFAULT_CODE_HEIGHT,
        "call_id": raw.get("call_id"),
        "approval_id": raw.get("approval_id"),
        "turn_id": raw.get("turn_id"),
        "cwd": raw.get("cwd"),
        "requester_pid": raw.get("requester_pid"),
        "command": raw.get("command"),
        "protocol_output_type": "exec_approval",
    }
    decisions = _default_exec_decisions(raw)
    payload["options"] = [
        _option_for_exec_decision(
            decision,
            network_approval_context=network_approval_context,
            additional_permissions=additional_permissions,
        )
        for decision in decisions
    ]
    return payload


def _normalize_patch_request(raw: dict[str, Any]) -> dict[str, Any]:
    code = _summarize_patch_changes(raw.get("changes", {}))
    options = [
        {
            "id": "approved",
            "label": "Yes, proceed",
            "key": "y",
            "default": True,
            "decision": "approved",
        },
        {
            "id": "approved_for_session",
            "label": "Yes, and don't ask again for these files",
            "key": "a",
            "decision": "approved_for_session",
        },
        {
            "id": "abort",
            "label": "No, and tell Codex what to do differently",
            "key": "n",
            "cancel": True,
            "decision": "abort",
        },
    ]
    return {
        "kind": "patch",
        "window_title": "Approval Request",
        "title": _default_title({"kind": "patch"}),
        "message": "Codex needs your approval before continuing.",
        "reason": raw.get("reason"),
        "thread_id": raw.get("thread_id"),
        "thread_label": raw.get("thread_label"),
        "thread": raw.get("thread_label"),
        "is_current_thread": raw.get("is_current_thread"),
        "current_thread_id": raw.get("current_thread_id"),
        "permission_rule": f"grant write access under `{raw['grant_root']}`" if raw.get("grant_root") else None,
        "code": code or None,
        "show_shortcuts_hint": False,
        "code_selectable": False,
        "width": DEFAULT_WIDTH,
        "code_height": DEFAULT_CODE_HEIGHT,
        "call_id": raw.get("call_id"),
        "turn_id": raw.get("turn_id"),
        "cwd": raw.get("cwd"),
        "requester_pid": raw.get("requester_pid"),
        "changes": raw.get("changes"),
        "protocol_output_type": "patch_approval",
        "options": options,
    }


def _normalize_elicitation_request(raw: dict[str, Any]) -> dict[str, Any]:
    options = [
        {
            "id": "accept",
            "label": "Yes, provide the requested info",
            "key": "y",
            "default": True,
            "decision": "accept",
        },
        {
            "id": "decline",
            "label": "No, but continue without it",
            "key": "a",
            "decision": "decline",
        },
        {
            "id": "cancel",
            "label": "Cancel this request",
            "key": "n",
            "cancel": True,
            "decision": "cancel",
        },
    ]
    return {
        "kind": "elicitation",
        "window_title": "Approval Request",
        "title": _default_title({"kind": "elicitation", "server_name": raw.get("server_name")}),
        "message": "Codex needs your approval before continuing.",
        "thread_id": raw.get("thread_id"),
        "thread_label": raw.get("thread_label"),
        "thread": raw.get("thread_label"),
        "is_current_thread": raw.get("is_current_thread"),
        "current_thread_id": raw.get("current_thread_id"),
        "server_name": raw.get("server_name"),
        "turn_id": raw.get("turn_id"),
        "cwd": raw.get("cwd"),
        "requester_pid": raw.get("requester_pid"),
        "code": raw.get("message"),
        "show_shortcuts_hint": False,
        "code_selectable": False,
        "width": DEFAULT_WIDTH,
        "code_height": DEFAULT_CODE_HEIGHT,
        "request_id": raw.get("id"),
        "protocol_output_type": "resolve_elicitation",
        "options": options,
    }


def _looks_like_exec_request(raw: dict[str, Any]) -> bool:
    return "call_id" in raw and "command" in raw and "cwd" in raw


def _looks_like_patch_request(raw: dict[str, Any]) -> bool:
    return "call_id" in raw and "changes" in raw and "command" not in raw


def _looks_like_elicitation_request(raw: dict[str, Any]) -> bool:
    return "server_name" in raw and "id" in raw and "message" in raw and "call_id" not in raw


def _default_options(kind: str) -> list[dict[str, Any]]:
    if kind == "patch":
        return [
            {"id": "abort", "label": "No", "key": "n", "cancel": True},
            {
                "id": "approved_for_session",
                "label": "Yes, and don't ask again for these files",
                "key": "a",
            },
            {"id": "approved", "label": "Yes, proceed", "key": "y", "default": True},
        ]
    if kind == "network":
        return [
            {"id": "abort", "label": "No", "key": "n", "cancel": True},
            {
                "id": "approved_for_session",
                "label": "Yes, and allow this host for this conversation",
                "key": "a",
            },
            {"id": "approved", "label": "Yes, just this once", "key": "y", "default": True},
        ]
    if kind == "elicitation":
        return [
            {"id": "cancel", "label": "Cancel this request", "key": "n", "cancel": True},
            {"id": "decline", "label": "No, but continue without it", "key": "a"},
            {"id": "accept", "label": "Yes, provide the requested info", "key": "y", "default": True},
        ]
    return [
        {"id": "abort", "label": "No", "key": "n", "cancel": True},
        {
            "id": "approved_for_session",
            "label": "Yes, and don't ask again this session",
            "key": "a",
        },
        {"id": "approved", "label": "Yes, proceed", "key": "y", "default": True},
    ]


def _default_title(payload: dict[str, Any]) -> str:
    kind = payload["kind"]
    if kind == "patch":
        return "Would you like to make the following edits?"
    if kind == "network":
        host = payload.get("host", "this host")
        return f'Do you want to approve network access to "{host}"?'
    if kind == "elicitation":
        server = payload.get("server_name", "Tool")
        return f"{server} needs your approval."
    return "Would you like to run the following command?"


def _default_message(_payload: dict[str, Any]) -> str:
    return "Codex needs your approval before continuing."


def normalize_payload(raw: dict[str, Any]) -> dict[str, Any]:
    if "requester_pid" not in raw:
        requester_pid = os.environ.get(REQUESTER_PID_ENV)
        if requester_pid:
            raw = dict(raw)
            raw["requester_pid"] = requester_pid
    if _looks_like_exec_request(raw):
        payload = _normalize_exec_request(raw)
    elif _looks_like_patch_request(raw):
        payload = _normalize_patch_request(raw)
    elif _looks_like_elicitation_request(raw):
        payload = _normalize_elicitation_request(raw)
    else:
        payload = dict(raw)
    payload.setdefault("kind", "exec")
    payload.setdefault("window_title", "Approval Request")
    payload.setdefault("title", _default_title(payload))
    payload.setdefault("message", _default_message(payload))
    payload.setdefault("options", _default_options(payload["kind"]))
    payload.setdefault("show_shortcuts_hint", False)
    payload.setdefault("code_selectable", False)
    payload.setdefault("width", DEFAULT_WIDTH)
    payload.setdefault("code_height", DEFAULT_CODE_HEIGHT)
    payload.setdefault("shortcut_enable_delay_ms", DEFAULT_SHORTCUT_ENABLE_DELAY_MS)

    code = payload.get("code")
    if code is not None and not isinstance(code, str):
        raise ValueError("`code` must be a string when present")
    shortcut_enable_delay_ms = payload.get("shortcut_enable_delay_ms")
    if not isinstance(shortcut_enable_delay_ms, int) or shortcut_enable_delay_ms < 0:
        raise ValueError("`shortcut_enable_delay_ms` must be a non-negative integer")

    options = payload["options"]
    if not isinstance(options, list) or not options:
        raise ValueError("`options` must be a non-empty list")

    seen_ids: set[str] = set()
    default_count = 0
    for option in options:
        if not isinstance(option, dict):
            raise ValueError("each option must be an object")
        option.setdefault("id", option["label"])
        if option["id"] in seen_ids:
            raise ValueError(f"duplicate option id: {option['id']}")
        seen_ids.add(option["id"])
        key = option.get("key")
        if key is not None and (not isinstance(key, str) or len(key) != 1):
            raise ValueError(f"option key must be a single character: {option!r}")
        if option.get("default"):
            default_count += 1

    if default_count > 1:
        raise ValueError("at most one option may be marked as default")

    return payload


def build_shortcuts_hint(options: list[dict[str, Any]]) -> str:
    parts = []
    for option in options:
        key = option.get("key")
        if key:
            parts.append(f"{key} = {option['label']}")
    return "Shortcuts: " + ", ".join(parts)


def _wrap_dialog_text(
    text: str,
    *,
    width: int,
    initial_indent: str = "",
    subsequent_indent: str | None = None,
) -> tuple[str, int]:
    if subsequent_indent is None:
        subsequent_indent = " " * len(initial_indent)
    wrapped = textwrap.fill(
        text,
        width=width,
        initial_indent=initial_indent,
        subsequent_indent=subsequent_indent,
    )
    return wrapped, wrapped.count("\n") + 1


def add_display_labels(options: list[dict[str, Any]]) -> None:
    for option in options:
        key = option.get("key")
        if key:
            prefix = f"({key}) "
            display_label = textwrap.fill(
                option["label"],
                width=BUTTON_WRAP_WIDTH,
                initial_indent=prefix,
                subsequent_indent=" " * (len(prefix) + 2),
            )
        else:
            display_label = textwrap.fill(
                option["label"],
                width=BUTTON_WRAP_WIDTH,
                subsequent_indent="    ",
            )
        option["display_label"] = display_label
        option["display_line_count"] = display_label.count("\n") + 1


def add_wrapped_dialog_fields(payload: dict[str, Any]) -> None:
    title_display, title_line_count = _wrap_dialog_text(
        payload["title"],
        width=TITLE_WRAP_WIDTH,
    )
    message_display, message_line_count = _wrap_dialog_text(
        payload["message"],
        width=BODY_WRAP_WIDTH,
    )
    payload["title_display"] = title_display
    payload["title_line_count"] = title_line_count
    payload["message_display"] = message_display
    payload["message_line_count"] = message_line_count

    detail_specs: list[tuple[str, str | None]] = [
        ("Thread: ", payload.get("thread")),
        (
            "Requester PID: ",
            str(payload["requester_pid"]) if payload.get("requester_pid") is not None else None,
        ),
        ("Working dir: ", payload.get("cwd")),
        ("Reason: ", payload.get("reason")),
        ("Permission rule: ", payload.get("permission_rule")),
        ("Host: ", payload.get("host") if payload.get("kind") == "network" else None),
        (
            "Server: ",
            payload.get("server_name") if payload.get("kind") == "elicitation" else None,
        ),
    ]
    detail_rows = []
    for prefix, value in detail_specs:
        if not value:
            continue
        text, line_count = _wrap_dialog_text(
            f"{prefix}{value}",
            width=BODY_WRAP_WIDTH,
            subsequent_indent=" " * len(prefix),
        )
        detail_rows.append({"text": text, "line_count": line_count})
    payload["detail_rows"] = detail_rows

    if payload.get("show_shortcuts_hint"):
        shortcuts_display, shortcuts_line_count = _wrap_dialog_text(
            payload["shortcuts_hint"],
            width=BODY_WRAP_WIDTH,
        )
        payload["shortcuts_display"] = shortcuts_display
        payload["shortcuts_line_count"] = shortcuts_line_count


def format_thread_summary(payload: dict[str, Any]) -> str | None:
    thread_label = payload.get("thread_label") or payload.get("thread")
    thread_id = payload.get("thread_id")
    is_current_thread = payload.get("is_current_thread")
    current_thread_id = payload.get("current_thread_id")

    if is_current_thread is None and thread_id is not None and current_thread_id is not None:
        is_current_thread = thread_id == current_thread_id

    if not thread_label and not thread_id:
        return None

    current_suffix = " (current)" if is_current_thread else ""
    if thread_label and thread_id:
        return f"{thread_label}{current_suffix}  {thread_id}"
    if thread_label:
        return f"{thread_label}{current_suffix}"
    return str(thread_id)


def _debug_dir() -> Path | None:
    debug_dir = os.environ.get(DEBUG_DIR_ENV)
    debug_enabled = os.environ.get(DEBUG_ENV)
    if debug_dir is None and debug_enabled != "1":
        return None
    base = Path(debug_dir) if debug_dir else Path(tempfile.gettempdir()) / "codex-approval-dialog"
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    path = base / f"{stamp}-{os.getpid()}"
    path.mkdir(parents=True, exist_ok=True)
    return path


def _write_debug_file(debug_dir: Path | None, name: str, content: str) -> None:
    if debug_dir is None:
        return
    (debug_dir / name).write_text(content, encoding="utf-8")


def _append_debug_event(debug_dir: Path | None, message: str) -> None:
    if debug_dir is None:
        return
    timestamp = datetime.now().isoformat(timespec="milliseconds")
    with (debug_dir / "timeline.log").open("a", encoding="utf-8") as handle:
        handle.write(f"{timestamp} {message}\n")


def _read_stdin_text(debug_dir: Path | None) -> str:
    stdin_fd = sys.stdin.fileno()
    chunks: list[bytes] = []
    total_bytes = 0
    timeout_ms = max(int(os.environ.get(STDIN_TIMEOUT_ENV, DEFAULT_STDIN_TIMEOUT_MS)), 1)
    _append_debug_event(debug_dir, "load_input:stdin_read:start")
    while True:
        readable, _, _ = select.select([stdin_fd], [], [], timeout_ms / 1000.0)
        if not readable:
            _append_debug_event(
                debug_dir,
                f"load_input:stdin_read:timeout total_bytes={total_bytes} timeout_ms={timeout_ms}",
            )
            if total_bytes > 0:
                _append_debug_event(
                    debug_dir,
                    f"load_input:stdin_read:progress_after_timeout total_bytes={total_bytes}",
                )
                break
            raise TimeoutError(
                f"stdin read timed out after {timeout_ms} ms waiting for input"
            )
        chunk = os.read(stdin_fd, 65536)
        if not chunk:
            _append_debug_event(
                debug_dir,
                f"load_input:stdin_read:eof total_bytes={total_bytes}",
            )
            break
        nul_index = chunk.find(b"\0")
        if nul_index >= 0:
            chunk = chunk[:nul_index]
            if chunk:
                chunks.append(chunk)
                total_bytes += len(chunk)
            _append_debug_event(
                debug_dir,
                f"load_input:stdin_read:nul_terminator total_bytes={total_bytes}",
            )
            break
        chunks.append(chunk)
        total_bytes += len(chunk)
        _append_debug_event(
            debug_dir,
            f"load_input:stdin_read:chunk bytes={len(chunk)} total_bytes={total_bytes}",
        )
    return b"".join(chunks).decode("utf-8")


_APPKIT = None
_FOUNDATION = None
_OBJC = None
_NATIVE_DIALOG_CONTROLLER = None


def _ensure_pyobjc():
    global _APPKIT, _FOUNDATION, _OBJC, _NATIVE_DIALOG_CONTROLLER

    if _APPKIT is None:
        try:
            import AppKit as appkit
            import Foundation as foundation
            import objc as objc_module
        except ImportError as exc:  # pragma: no cover - environment-specific
            raise RuntimeError(
                "PyObjC/AppKit is required to show the approval dialog"
            ) from exc
        _APPKIT = appkit
        _FOUNDATION = foundation
        _OBJC = objc_module

    if _NATIVE_DIALOG_CONTROLLER is None:
        AppKit = _APPKIT
        Foundation = _FOUNDATION
        objc = _OBJC

        class NativeApprovalDialogController(Foundation.NSObject):
            @objc.python_method
            def configure(self, payload: dict[str, Any], debug_dir: Path | None) -> None:
                self.payload = payload
                self.debug_dir = debug_dir
                self.window = None
                self.event_monitor = None
                self.enable_timer = None
                self.buttons = []
                self.selection_index = None
                self.response_code = None
                self.shortcut_keys = {
                    str(option["key"]).lower()
                    for option in payload["options"]
                    if option.get("key")
                }
                delay_ms = payload.get("shortcut_enable_delay_ms", 0)
                self.shortcut_enable_time = time.monotonic() + (delay_ms / 1000.0)
                self.shortcut_delay_seconds = delay_ms / 1000.0

            @objc.python_method
            def log(self, message: str) -> None:
                _append_debug_event(self.debug_dir, f"native_dialog:{message}")

            @objc.python_method
            def set_buttons_enabled(self, enabled: bool) -> None:
                for button in self.buttons:
                    button.setEnabled_(enabled)
                    button.setAlphaValue_(1.0 if enabled else 0.55)
                self.log(f"buttons:enabled={enabled}")

            @objc.python_method
            def handle_key_event(self, event):
                if self.window is None:
                    return event
                event_window = event.window()
                if event_window is not None and event_window != self.window:
                    return event
                chars = event.charactersIgnoringModifiers()
                key = str(chars).lower() if chars is not None else ""
                if key in {"\r", "\n"}:
                    remaining = self.shortcut_enable_time - time.monotonic()
                    if remaining > 0:
                        self.log(f"eventMonitor:ignored return remaining={remaining:.3f}")
                        return None
                    default_index = next(
                        (
                            index
                            for index, option in enumerate(self.payload["options"])
                            if option.get("default")
                        ),
                        None,
                    )
                    if default_index is None:
                        return event
                    self.log(f"eventMonitor:allowed return default_index={default_index}")
                    self.buttonPressed_(self.buttons[default_index])
                    return None
                if key not in self.shortcut_keys:
                    return event
                remaining = self.shortcut_enable_time - time.monotonic()
                if remaining > 0:
                    self.log(f"eventMonitor:ignored key={key} remaining={remaining:.3f}")
                    return None
                self.log(f"eventMonitor:allowed key={key}")
                return event

            @objc.python_method
            def install_event_monitor(self) -> None:
                self.log("eventMonitor:installing")
                self.event_monitor = AppKit.NSEvent.addLocalMonitorForEventsMatchingMask_handler_(
                    AppKit.NSEventMaskKeyDown,
                    self.handle_key_event,
                )
                self.log("eventMonitor:installed")

            @objc.python_method
            def remove_event_monitor(self) -> None:
                if self.event_monitor is not None:
                    AppKit.NSEvent.removeMonitor_(self.event_monitor)
                    self.event_monitor = None
                    self.log("eventMonitor:removed")

            @objc.python_method
            def install_enable_timer(self) -> None:
                if self.shortcut_delay_seconds <= 0:
                    self.set_buttons_enabled(True)
                    return
                self.set_buttons_enabled(False)
                self.log(f"buttons:enable_timer_installing delay={self.shortcut_delay_seconds:.3f}")
                self.enable_timer = Foundation.NSTimer.timerWithTimeInterval_target_selector_userInfo_repeats_(
                    self.shortcut_delay_seconds,
                    self,
                    "enableButtonsFromTimer:",
                    None,
                    False,
                )
                run_loop = Foundation.NSRunLoop.currentRunLoop()
                run_loop.addTimer_forMode_(self.enable_timer, AppKit.NSModalPanelRunLoopMode)
                run_loop.addTimer_forMode_(self.enable_timer, Foundation.NSRunLoopCommonModes)
                self.log("buttons:enable_timer_installed")

            @objc.python_method
            def remove_enable_timer(self) -> None:
                if self.enable_timer is not None:
                    self.enable_timer.invalidate()
                    self.enable_timer = None
                    self.log("buttons:enable_timer_removed")

            def enableButtonsFromTimer_(self, _timer) -> None:
                self.enable_timer = None
                self.set_buttons_enabled(True)
                self.log("buttons:enable_timer_fired")

            @objc.IBAction
            def buttonPressed_(self, sender) -> None:
                self.selection_index = int(sender.tag())
                self.response_code = 1000 + self.selection_index
                self.log(f"buttonPressed index={self.selection_index}")
                AppKit.NSApplication.sharedApplication().stopModalWithCode_(self.response_code)
                if self.window is not None:
                    self.window.orderOut_(None)

        _NATIVE_DIALOG_CONTROLLER = NativeApprovalDialogController

    return _APPKIT, _FOUNDATION, _OBJC, _NATIVE_DIALOG_CONTROLLER


def _make_native_label(AppKit, text: str, width: float, height: float, font, *, selectable: bool = False):
    field = AppKit.NSTextField.alloc().initWithFrame_(AppKit.NSMakeRect(0, 0, width, height))
    field.setStringValue_(text)
    field.setEditable_(False)
    field.setSelectable_(selectable)
    field.setBezeled_(False)
    field.setBordered_(False)
    field.setDrawsBackground_(False)
    field.setLineBreakMode_(AppKit.NSLineBreakByWordWrapping)
    field.setUsesSingleLineMode_(False)
    field.setFont_(font)
    return field


def _native_dialog_heights(payload: dict[str, Any]) -> tuple[int, list[int]]:
    title_height = max(22, payload.get("title_line_count", 1) * 20)
    message_height = max(18, payload.get("message_line_count", 1) * 17)
    details_height = sum(max(18, row["line_count"] * 17) + 6 for row in payload["detail_rows"])
    shortcuts_height = (
        max(18, payload.get("shortcuts_line_count", 1) * 17) + 8
        if payload.get("show_shortcuts_hint")
        else 0
    )
    code_height = payload["code_height"] + 10 if payload.get("code") else 0
    button_heights = []
    for option in payload["options"]:
        text_height = max(18, option.get("display_line_count", 1) * 17)
        button_heights.append(max(NATIVE_BUTTON_MIN_HEIGHT, text_height + NATIVE_BUTTON_VERTICAL_PADDING))
    content_height = (
        18
        + title_height
        + 8
        + message_height
        + 12
        + details_height
        + (code_height + 12 if payload.get("code") else 0)
        + shortcuts_height
        + sum(button_heights)
        + max(0, len(button_heights) - 1) * BUTTON_VERTICAL_GAP
        + 18
    )
    return content_height, button_heights


def _build_native_window(payload: dict[str, Any], controller, AppKit):
    content_height, button_heights = _native_dialog_heights(payload)
    width = payload["width"]
    inner_width = width - 36
    title_font = AppKit.NSFont.boldSystemFontOfSize_(16)
    body_font = AppKit.NSFont.systemFontOfSize_(13)
    button_font = AppKit.NSFont.systemFontOfSize_(13)

    window = AppKit.NSWindow.alloc().initWithContentRect_styleMask_backing_defer_(
        AppKit.NSMakeRect(0, 0, width, content_height),
        AppKit.NSWindowStyleMaskTitled,
        AppKit.NSBackingStoreBuffered,
        False,
    )
    window.setTitle_(payload["window_title"])
    window.setOpaque_(True)
    window.setAlphaValue_(1.0)
    window.setBackgroundColor_(AppKit.NSColor.windowBackgroundColor())
    window.setTitlebarAppearsTransparent_(False)
    window.setMovableByWindowBackground_(False)
    window.setLevel_(AppKit.NSModalPanelWindowLevel)
    window.center()
    window.setReleasedWhenClosed_(False)
    controller.window = window

    content_view = window.contentView()
    y = content_height - 18

    title_height = max(22, payload.get("title_line_count", 1) * 20)
    title_field = _make_native_label(
        AppKit,
        payload.get("title_display", payload["title"]),
        inner_width,
        title_height,
        title_font,
    )
    y -= title_height
    title_field.setFrame_(AppKit.NSMakeRect(18, y, inner_width, title_height))
    content_view.addSubview_(title_field)
    y -= 8

    message_height = max(18, payload.get("message_line_count", 1) * 17)
    message_field = _make_native_label(
        AppKit,
        payload.get("message_display", payload["message"]),
        inner_width,
        message_height,
        body_font,
    )
    y -= message_height
    message_field.setFrame_(AppKit.NSMakeRect(18, y, inner_width, message_height))
    content_view.addSubview_(message_field)
    y -= 12

    for row in payload["detail_rows"]:
        row_height = max(18, row["line_count"] * 17)
        row_field = _make_native_label(AppKit, row["text"], inner_width, row_height, body_font)
        y -= row_height
        row_field.setFrame_(AppKit.NSMakeRect(18, y, inner_width, row_height))
        content_view.addSubview_(row_field)
        y -= 6

    if payload.get("code"):
        scroll_y = y - payload["code_height"]
        text_view = AppKit.NSTextView.alloc().initWithFrame_(
            AppKit.NSMakeRect(0, 0, inner_width, payload["code_height"])
        )
        text_view.setEditable_(False)
        text_view.setSelectable_(bool(payload.get("code_selectable")))
        text_view.setRichText_(False)
        text_view.setImportsGraphics_(False)
        text_view.setUsesFindBar_(True)
        text_view.setFont_(AppKit.NSFont.userFixedPitchFontOfSize_(12))
        text_view.textContainer().setWidthTracksTextView_(True)
        text_view.textContainer().setContainerSize_(AppKit.NSMakeSize(inner_width, 10_000_000))
        text_view.setHorizontallyResizable_(False)
        text_view.setVerticallyResizable_(True)
        text_view.setMaxSize_(AppKit.NSMakeSize(inner_width, 10_000_000))
        text_view.setString_(payload["code"])

        scroll_view = AppKit.NSScrollView.alloc().initWithFrame_(
            AppKit.NSMakeRect(18, scroll_y, inner_width, payload["code_height"])
        )
        scroll_view.setBorderType_(AppKit.NSBezelBorder)
        scroll_view.setHasVerticalScroller_(True)
        scroll_view.setHasHorizontalScroller_(False)
        scroll_view.setAutohidesScrollers_(True)
        scroll_view.setDocumentView_(text_view)
        content_view.addSubview_(scroll_view)
        y = scroll_y - 12

    if payload.get("show_shortcuts_hint"):
        hint_height = max(18, payload.get("shortcuts_line_count", 1) * 17)
        hint_field = _make_native_label(
            AppKit,
            payload.get("shortcuts_display", payload["shortcuts_hint"]),
            inner_width,
            hint_height,
            body_font,
        )
        y -= hint_height
        hint_field.setFrame_(AppKit.NSMakeRect(18, y, inner_width, hint_height))
        content_view.addSubview_(hint_field)
        y -= 8

    default_index = next(
        (index for index, option in enumerate(payload["options"]) if option.get("default")),
        -1,
    )
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", category=_OBJC.ObjCPointerWarning)
        accent_color = AppKit.NSColor.controlAccentColor().CGColor()
        control_color = AppKit.NSColor.controlColor().CGColor()
        separator_color = AppKit.NSColor.separatorColor().CGColor()
    buttons = []
    for index, option in enumerate(payload["options"]):
        button_height = button_heights[index]
        label_height = max(18, option.get("display_line_count", 1) * 17)
        y -= button_height
        button = AppKit.NSButton.alloc().initWithFrame_(
            AppKit.NSMakeRect(18, y, inner_width, button_height)
        )
        button.setTitle_("")
        button.setTag_(index)
        button.setTarget_(controller)
        button.setAction_("buttonPressed:")
        button.setBordered_(False)
        button.setWantsLayer_(True)
        button.layer().setCornerRadius_(7)
        if index == default_index:
            button.layer().setBackgroundColor_(accent_color)
        else:
            button.layer().setBackgroundColor_(control_color)
            button.layer().setBorderWidth_(1)
            button.layer().setBorderColor_(separator_color)
        button.setFont_(button_font)
        if option.get("key"):
            button.setKeyEquivalent_(option["key"])
            button.setKeyEquivalentModifierMask_(0)

        button_label = _make_native_label(
            AppKit,
            option.get("display_label", option["label"]),
            inner_width - 28,
            label_height,
            button_font,
        )
        if index == default_index:
            button_label.setTextColor_(AppKit.NSColor.alternateSelectedControlTextColor())
        label_y = max(9, int(round((button_height - label_height) / 2)))
        button_label.setFrame_(AppKit.NSMakeRect(14, label_y, inner_width - 28, label_height))
        button.addSubview_(button_label)

        content_view.addSubview_(button)
        buttons.append(button)
        if index + 1 < len(payload["options"]):
            y -= BUTTON_VERTICAL_GAP

    controller.buttons = buttons
    controller.log(f"buttons:created count={len(buttons)}")
    return window


def _run_native_dialog(payload: dict[str, Any], debug_dir: Path | None) -> dict[str, Any]:
    AppKit, _, _, Controller = _ensure_pyobjc()
    controller = Controller.alloc().init()
    controller.configure(payload, debug_dir)
    _write_debug_file(debug_dir, "dialog-mode.txt", "pyobjc\n")
    _write_debug_file(debug_dir, "normalized-payload.json", json.dumps(payload, indent=2))

    app = AppKit.NSApplication.sharedApplication()
    app.setActivationPolicy_(AppKit.NSApplicationActivationPolicyRegular)
    AppKit.NSRunningApplication.currentApplication().activateWithOptions_(
        AppKit.NSApplicationActivateIgnoringOtherApps
    )
    app.activateIgnoringOtherApps_(True)
    window = _build_native_window(payload, controller, AppKit)
    window.makeKeyAndOrderFront_(None)
    AppKit.NSRunningApplication.currentApplication().activateWithOptions_(
        AppKit.NSApplicationActivateIgnoringOtherApps | AppKit.NSApplicationActivateAllWindows
    )
    app.activateIgnoringOtherApps_(True)
    controller.log("window:visible")
    controller.log(
        "shortcuts:guard_active_until_monotonic="
        f"{controller.shortcut_enable_time:.6f}"
    )
    controller.install_event_monitor()
    controller.install_enable_timer()
    response = None
    try:
        response = app.runModalForWindow_(window)
        controller.log(f"runModal:return response={response}")
    finally:
        controller.remove_enable_timer()
        controller.remove_event_monitor()

    response_index = controller.selection_index
    if response_index is None or response_index < 0:
        response_index = next(
            (index for index, option in enumerate(payload["options"]) if option.get("cancel")),
            next(
                (index for index, option in enumerate(payload["options"]) if option.get("default")),
                0,
            ),
        )

    option = payload["options"][response_index]
    response_code = controller.response_code if controller.response_code is not None else response
    controller.log(f"selection:final index={response_index} id={option['id']}")
    return {
        "thread_id": payload.get("thread_id"),
        "thread_label": payload.get("thread_label"),
        "call_id": payload.get("call_id"),
        "approval_id": payload.get("approval_id"),
        "turn_id": payload.get("turn_id"),
        "id": option["id"],
        "label": option["label"],
        "key": option.get("key"),
        "decision": option.get("decision"),
        "response_index": response_index,
        "response_code": None if response_code is None else str(response_code),
    }


def run_dialog(payload: dict[str, Any], debug_dir: Path | None) -> dict[str, Any]:
    _append_debug_event(debug_dir, "run_dialog:start")
    _write_debug_file(debug_dir, "run-status.txt", "starting\n")
    try:
        selection = _run_native_dialog(payload, debug_dir)
    finally:
        _write_debug_file(debug_dir, "run-status.txt", "completed\n")
    _append_debug_event(debug_dir, "run_dialog:completed")
    return selection


def build_protocol_output(payload: dict[str, Any], selection: dict[str, Any]) -> dict[str, Any]:
    protocol_output_type = payload.get("protocol_output_type")
    decision = selection.get("decision")

    if protocol_output_type == "exec_approval":
        approval_id = payload.get("approval_id") or payload.get("call_id")
        if not approval_id:
            raise ValueError("exec approval payload is missing approval_id/call_id")
        result = {
            "type": "exec_approval",
            "id": approval_id,
            "decision": decision,
        }
        turn_id = payload.get("turn_id")
        if turn_id is not None:
            result["turn_id"] = turn_id
        return result

    if protocol_output_type == "patch_approval":
        call_id = payload.get("call_id")
        if not call_id:
            raise ValueError("patch approval payload is missing call_id")
        return {
            "type": "patch_approval",
            "id": call_id,
            "decision": decision,
        }

    if protocol_output_type == "resolve_elicitation":
        server_name = payload.get("server_name")
        request_id = payload.get("request_id")
        if server_name is None or request_id is None:
            raise ValueError("elicitation payload is missing server_name or request_id")
        return {
            "type": "resolve_elicitation",
            "server_name": server_name,
            "request_id": request_id,
            "decision": decision,
        }

    return selection


def build_test_payload() -> dict[str, Any]:
    return {
        "kind": "exec",
        "window_title": "Approval Request",
        "title": "Would you like to run the following command?",
        "message": "Codex needs your approval before continuing.",
        "thread_label": "Main [default]",
        "thread_id": "test-thread-id",
        "is_current_thread": True,
        "reason": "Smoke-test the custom macOS approval dialog renderer.",
        "cwd": "/Users/ebrevdo/code/codex",
        "code": (
            "python -m pytest tests/test_example.py "
            "--maxfail=1 -q --some-very-long-flag=value "
            "--another-long-flag='wrapped command text should stay inside the code box'"
        ),
        "protocol_output_type": "exec_approval",
        "approval_id": "test-approval-id",
        "turn_id": "test-turn-id",
        "shortcut_enable_delay_ms": DEFAULT_SHORTCUT_ENABLE_DELAY_MS,
        "options": [
            {
                "id": "approved",
                "label": "Yes, proceed",
                "key": "y",
                "default": True,
                "decision": "approved",
            },
            {
                "id": "approved_for_session",
                "label": "Yes, and do not ask again for this exact command during this session even if it is requested repeatedly from the same approval flow or a closely related one",
                "key": "a",
                "decision": "approved_for_session",
            },
            {
                "id": "abort",
                "label": "No, and tell Codex what to do differently",
                "key": "n",
                "cancel": True,
                "decision": "abort",
            },
        ],
    }


def load_input(path: str | None, *, test_mode: bool, debug_dir: Path | None) -> dict[str, Any]:
    if path:
        return json.loads(Path(path).read_text())
    if test_mode:
        if sys.stdin.isatty():
            return build_test_payload()
        readable, _, _ = select.select([sys.stdin], [], [], 0)
        if not readable:
            return build_test_payload()
    raw_text = _read_stdin_text(debug_dir)
    if test_mode and not raw_text.strip():
        return build_test_payload()
    return json.loads(raw_text)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", help="Path to the JSON request payload")
    parser.add_argument(
        "--normalize-only",
        action="store_true",
        help="Print the normalized request JSON and exit without running it",
    )
    parser.add_argument(
        "--test",
        action="store_true",
        help="Run a dialog smoke test; if no input is provided on stdin or via --input, use a built-in sample payload",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    debug_dir = _debug_dir()
    if debug_dir is None and args.test:
        debug_dir = Path(tempfile.gettempdir()) / "codex-approval-dialog" / "test"
        debug_dir.mkdir(parents=True, exist_ok=True)
    try:
        _append_debug_event(debug_dir, "main:start")
        raw = load_input(args.input, test_mode=args.test, debug_dir=debug_dir)
        _append_debug_event(debug_dir, "main:input_loaded")
        _write_debug_file(debug_dir, "raw-input.json", json.dumps(raw, indent=2))
        payload = normalize_payload(raw)
        _append_debug_event(debug_dir, "main:payload_normalized")
        payload["thread"] = format_thread_summary(payload)
        add_display_labels(payload["options"])
        payload["shortcuts_hint"] = build_shortcuts_hint(payload["options"])
        add_wrapped_dialog_fields(payload)
        if args.normalize_only:
            _append_debug_event(debug_dir, "main:normalize_only")
            json.dump(payload, sys.stdout, indent=2)
            sys.stdout.write("\n")
            return 0
        _append_debug_event(debug_dir, "main:run_dialog")
        selection = run_dialog(payload, debug_dir)
        _append_debug_event(debug_dir, "main:dialog_selection_received")
        protocol_output = build_protocol_output(payload, selection)
        result = (
            {"selection": selection, "protocol_output": protocol_output}
            if args.test
            else protocol_output
        )
        _append_debug_event(debug_dir, "main:protocol_output_built")
    except Exception as exc:  # pragma: no cover - CLI error path
        _append_debug_event(debug_dir, f"main:error {exc}")
        _write_debug_file(debug_dir, "python-error.txt", f"{exc}\n")
        json.dump({"error": str(exc)}, sys.stderr)
        sys.stderr.write("\n")
        return 1

    _append_debug_event(debug_dir, "main:success")
    json.dump(result, sys.stdout)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
