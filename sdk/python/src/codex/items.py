from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from typing import Iterable, Literal, Sequence, cast

from .exceptions import CodexError


class CommandExecutionStatus(StrEnum):
    IN_PROGRESS = "in_progress"
    COMPLETED = "completed"
    FAILED = "failed"


class PatchChangeKind(StrEnum):
    ADD = "add"
    DELETE = "delete"
    UPDATE = "update"


class PatchApplyStatus(StrEnum):
    COMPLETED = "completed"
    FAILED = "failed"


class McpToolCallStatus(StrEnum):
    IN_PROGRESS = "in_progress"
    COMPLETED = "completed"
    FAILED = "failed"


@dataclass(frozen=True, slots=True)
class CommandExecutionItem:
    type: Literal["command_execution"] = field(default="command_execution", init=False)
    id: str
    command: str
    aggregated_output: str
    status: CommandExecutionStatus
    exit_code: int | None = None


@dataclass(frozen=True, slots=True)
class FileUpdateChange:
    path: str
    kind: PatchChangeKind


@dataclass(frozen=True, slots=True)
class FileChangeItem:
    type: Literal["file_change"] = field(default="file_change", init=False)
    id: str
    changes: Sequence[FileUpdateChange]
    status: PatchApplyStatus


@dataclass(frozen=True, slots=True)
class McpToolCallItem:
    type: Literal["mcp_tool_call"] = field(default="mcp_tool_call", init=False)
    id: str
    server: str
    tool: str
    status: McpToolCallStatus


@dataclass(frozen=True, slots=True)
class AgentMessageItem:
    type: Literal["agent_message"] = field(default="agent_message", init=False)
    id: str
    text: str


@dataclass(frozen=True, slots=True)
class ReasoningItem:
    type: Literal["reasoning"] = field(default="reasoning", init=False)
    id: str
    text: str


@dataclass(frozen=True, slots=True)
class WebSearchItem:
    type: Literal["web_search"] = field(default="web_search", init=False)
    id: str
    query: str


@dataclass(frozen=True, slots=True)
class ErrorItem:
    type: Literal["error"] = field(default="error", init=False)
    id: str
    message: str


@dataclass(frozen=True, slots=True)
class TodoItem:
    text: str
    completed: bool


@dataclass(frozen=True, slots=True)
class TodoListItem:
    type: Literal["todo_list"] = field(default="todo_list", init=False)
    id: str
    items: Sequence[TodoItem]


ThreadItem = (
    AgentMessageItem
    | ReasoningItem
    | CommandExecutionItem
    | FileChangeItem
    | McpToolCallItem
    | WebSearchItem
    | TodoListItem
    | ErrorItem
)


def _ensure_str(value: object, field: str) -> str:
    if isinstance(value, str):
        return value
    raise CodexError(f"Expected string for {field}")


def _ensure_sequence(value: object, field: str) -> Sequence[object]:
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        return cast(Sequence[object], value)
    raise CodexError(f"Expected sequence for {field}")


def _parse_changes(values: Iterable[object]) -> list[FileUpdateChange]:
    changes: list[FileUpdateChange] = []
    for value in values:
        if not isinstance(value, dict):
            raise CodexError("Invalid file change entry")
        path = _ensure_str(value.get("path"), "path")
        kind = _ensure_str(value.get("kind"), "kind")
        try:
            enum_kind = PatchChangeKind(kind)
        except ValueError as exc:
            raise CodexError(f"Unsupported file change kind: {kind}") from exc
        changes.append(FileUpdateChange(path=path, kind=enum_kind))
    return changes


def _parse_todos(values: Iterable[object]) -> list[TodoItem]:
    todos: list[TodoItem] = []
    for value in values:
        if not isinstance(value, dict):
            raise CodexError("Invalid todo entry")
        text = _ensure_str(value.get("text"), "text")
        completed = bool(value.get("completed", False))
        todos.append(TodoItem(text=text, completed=completed))
    return todos


def parse_thread_item(payload: object) -> ThreadItem:
    if not isinstance(payload, dict):
        raise CodexError("Thread item must be an object")

    type_name = _ensure_str(payload.get("type"), "type")
    item_id = _ensure_str(payload.get("id"), "id")

    if type_name == "agent_message":
        text = _ensure_str(payload.get("text"), "text")
        return AgentMessageItem(id=item_id, text=text)

    if type_name == "reasoning":
        text = _ensure_str(payload.get("text"), "text")
        return ReasoningItem(id=item_id, text=text)

    if type_name == "command_execution":
        command = _ensure_str(payload.get("command"), "command")
        aggregated_output = _ensure_str(payload.get("aggregated_output"), "aggregated_output")
        status_str = _ensure_str(payload.get("status"), "status")
        try:
            status = CommandExecutionStatus(status_str)
        except ValueError as exc:
            raise CodexError(f"Unsupported command execution status: {status_str}") from exc
        exit_code = payload.get("exit_code")
        exit_value = int(exit_code) if isinstance(exit_code, int) else None
        return CommandExecutionItem(
            id=item_id,
            command=command,
            aggregated_output=aggregated_output,
            status=status,
            exit_code=exit_value,
        )

    if type_name == "file_change":
        changes_raw = _ensure_sequence(payload.get("changes"), "changes")
        status_str = _ensure_str(payload.get("status"), "status")
        try:
            change_status = PatchApplyStatus(status_str)
        except ValueError as exc:
            raise CodexError(f"Unsupported file change status: {status_str}") from exc
        changes = _parse_changes(changes_raw)
        return FileChangeItem(id=item_id, changes=changes, status=change_status)

    if type_name == "mcp_tool_call":
        server = _ensure_str(payload.get("server"), "server")
        tool = _ensure_str(payload.get("tool"), "tool")
        status_str = _ensure_str(payload.get("status"), "status")
        try:
            call_status = McpToolCallStatus(status_str)
        except ValueError as exc:
            raise CodexError(f"Unsupported MCP tool call status: {status_str}") from exc
        return McpToolCallItem(
            id=item_id,
            server=server,
            tool=tool,
            status=call_status,
        )

    if type_name == "web_search":
        query = _ensure_str(payload.get("query"), "query")
        return WebSearchItem(id=item_id, query=query)

    if type_name == "error":
        message = _ensure_str(payload.get("message"), "message")
        return ErrorItem(id=item_id, message=message)

    if type_name == "todo_list":
        todos_raw = _ensure_sequence(payload.get("items"), "items")
        todos = _parse_todos(todos_raw)
        return TodoListItem(id=item_id, items=todos)

    raise CodexError(f"Unsupported item type: {type_name}")
