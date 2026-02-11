from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal, Mapping, TypeAlias

CommandExecutionStatus: TypeAlias = Literal["in_progress", "completed", "failed"]
PatchChangeKind: TypeAlias = Literal["add", "delete", "update"]
PatchApplyStatus: TypeAlias = Literal["completed", "failed"]
McpToolCallStatus: TypeAlias = Literal["in_progress", "completed", "failed"]


@dataclass(slots=True)
class CommandExecutionItem:
    id: str
    command: str
    aggregated_output: str
    status: CommandExecutionStatus
    exit_code: int | None = None
    type: Literal["command_execution"] = "command_execution"


@dataclass(slots=True)
class FileUpdateChange:
    path: str
    kind: PatchChangeKind


@dataclass(slots=True)
class FileChangeItem:
    id: str
    changes: list[FileUpdateChange]
    status: PatchApplyStatus
    type: Literal["file_change"] = "file_change"


@dataclass(slots=True)
class McpToolCallResult:
    content: list[Any]
    structured_content: Any


@dataclass(slots=True)
class McpToolCallError:
    message: str


@dataclass(slots=True)
class McpToolCallItem:
    id: str
    server: str
    tool: str
    arguments: Any
    status: McpToolCallStatus
    result: McpToolCallResult | None = None
    error: McpToolCallError | None = None
    type: Literal["mcp_tool_call"] = "mcp_tool_call"


@dataclass(slots=True)
class AgentMessageItem:
    id: str
    text: str
    type: Literal["agent_message"] = "agent_message"


@dataclass(slots=True)
class ReasoningItem:
    id: str
    text: str
    type: Literal["reasoning"] = "reasoning"


@dataclass(slots=True)
class WebSearchItem:
    id: str
    query: str
    type: Literal["web_search"] = "web_search"


@dataclass(slots=True)
class ErrorItem:
    id: str
    message: str
    type: Literal["error"] = "error"


@dataclass(slots=True)
class TodoItem:
    text: str
    completed: bool


@dataclass(slots=True)
class TodoListItem:
    id: str
    items: list[TodoItem] = field(default_factory=list)
    type: Literal["todo_list"] = "todo_list"


ThreadItem: TypeAlias = (
    AgentMessageItem
    | ReasoningItem
    | CommandExecutionItem
    | FileChangeItem
    | McpToolCallItem
    | WebSearchItem
    | TodoListItem
    | ErrorItem
)


def parse_thread_item(raw: Mapping[str, Any]) -> ThreadItem:
    item_type = str(raw.get("type", ""))

    if item_type == "agent_message":
        return AgentMessageItem(id=str(raw.get("id", "")), text=str(raw.get("text", "")))
    if item_type == "reasoning":
        return ReasoningItem(id=str(raw.get("id", "")), text=str(raw.get("text", "")))
    if item_type == "command_execution":
        exit_code = raw.get("exit_code")
        parsed_exit_code = int(exit_code) if isinstance(exit_code, int) else None
        return CommandExecutionItem(
            id=str(raw.get("id", "")),
            command=str(raw.get("command", "")),
            aggregated_output=str(raw.get("aggregated_output", "")),
            status=str(raw.get("status", "in_progress")),  # type: ignore[arg-type]
            exit_code=parsed_exit_code,
        )
    if item_type == "file_change":
        changes: list[FileUpdateChange] = []
        raw_changes = raw.get("changes", [])
        if isinstance(raw_changes, list):
            for change in raw_changes:
                if isinstance(change, Mapping):
                    changes.append(
                        FileUpdateChange(
                            path=str(change.get("path", "")),
                            kind=str(change.get("kind", "update")),  # type: ignore[arg-type]
                        )
                    )
        return FileChangeItem(
            id=str(raw.get("id", "")),
            changes=changes,
            status=str(raw.get("status", "failed")),  # type: ignore[arg-type]
        )
    if item_type == "mcp_tool_call":
        result: McpToolCallResult | None = None
        error: McpToolCallError | None = None

        raw_result = raw.get("result")
        if isinstance(raw_result, Mapping):
            raw_content = raw_result.get("content", [])
            content = list(raw_content) if isinstance(raw_content, list) else []
            result = McpToolCallResult(
                content=content,
                structured_content=raw_result.get("structured_content"),
            )

        raw_error = raw.get("error")
        if isinstance(raw_error, Mapping):
            error = McpToolCallError(message=str(raw_error.get("message", "")))

        return McpToolCallItem(
            id=str(raw.get("id", "")),
            server=str(raw.get("server", "")),
            tool=str(raw.get("tool", "")),
            arguments=raw.get("arguments"),
            result=result,
            error=error,
            status=str(raw.get("status", "in_progress")),  # type: ignore[arg-type]
        )
    if item_type == "web_search":
        return WebSearchItem(id=str(raw.get("id", "")), query=str(raw.get("query", "")))
    if item_type == "todo_list":
        todos: list[TodoItem] = []
        raw_items = raw.get("items", [])
        if isinstance(raw_items, list):
            for todo in raw_items:
                if isinstance(todo, Mapping):
                    todos.append(
                        TodoItem(
                            text=str(todo.get("text", "")),
                            completed=bool(todo.get("completed", False)),
                        )
                    )
        return TodoListItem(id=str(raw.get("id", "")), items=todos)
    if item_type == "error":
        return ErrorItem(id=str(raw.get("id", "")), message=str(raw.get("message", "")))

    raise ValueError(f"Unknown thread item type: {item_type}")
