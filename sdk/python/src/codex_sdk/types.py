from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Union

JsonDict = Dict[str, Any]


@dataclass
class Usage:
    input_tokens: int = 0
    cached_input_tokens: int = 0
    output_tokens: int = 0


@dataclass
class ThreadError:
    message: str


@dataclass
class AgentMessageItem:
    id: str
    type: str
    text: str


@dataclass
class ReasoningItem:
    id: str
    type: str
    text: str


@dataclass
class CommandExecutionItem:
    id: str
    type: str
    command: str
    aggregated_output: str
    status: str
    exit_code: Optional[int] = None


@dataclass
class FileUpdateChange:
    path: str
    kind: str


@dataclass
class FileChangeItem:
    id: str
    type: str
    changes: List[FileUpdateChange]
    status: str


@dataclass
class McpToolCallResult:
    content: List[JsonDict]
    structured_content: Any


@dataclass
class McpToolCallError:
    message: str


@dataclass
class McpToolCallItem:
    id: str
    type: str
    server: str
    tool: str
    arguments: Any
    status: str
    result: Optional[McpToolCallResult] = None
    error: Optional[McpToolCallError] = None


@dataclass
class WebSearchItem:
    id: str
    type: str
    query: str


@dataclass
class TodoItem:
    text: str
    completed: bool


@dataclass
class TodoListItem:
    id: str
    type: str
    items: List[TodoItem]


@dataclass
class ErrorItem:
    id: str
    type: str
    message: str


ThreadItem = Union[
    AgentMessageItem,
    ReasoningItem,
    CommandExecutionItem,
    FileChangeItem,
    McpToolCallItem,
    WebSearchItem,
    TodoListItem,
    ErrorItem,
]


@dataclass
class ThreadStartedEvent:
    type: str
    thread_id: str


@dataclass
class TurnStartedEvent:
    type: str


@dataclass
class TurnCompletedEvent:
    type: str
    usage: Usage


@dataclass
class TurnFailedEvent:
    type: str
    error: ThreadError


@dataclass
class ItemStartedEvent:
    type: str
    item: ThreadItem


@dataclass
class ItemUpdatedEvent:
    type: str
    item: ThreadItem


@dataclass
class ItemCompletedEvent:
    type: str
    item: ThreadItem


@dataclass
class ThreadErrorEvent:
    type: str
    message: str


ThreadEvent = Union[
    ThreadStartedEvent,
    TurnStartedEvent,
    TurnCompletedEvent,
    TurnFailedEvent,
    ItemStartedEvent,
    ItemUpdatedEvent,
    ItemCompletedEvent,
    ThreadErrorEvent,
]


def parse_thread_event(payload: Union[str, JsonDict]) -> ThreadEvent:
    data = json.loads(payload) if isinstance(payload, str) else dict(payload)
    event_type = data.get("type")
    if event_type == "thread.started":
        return ThreadStartedEvent(type=event_type, thread_id=str(data["thread_id"]))
    if event_type == "turn.started":
        return TurnStartedEvent(type=event_type)
    if event_type == "turn.completed":
        usage_data = data.get("usage", {}) or {}
        usage = Usage(
            input_tokens=int(usage_data.get("input_tokens", 0) or 0),
            cached_input_tokens=int(usage_data.get("cached_input_tokens", 0) or 0),
            output_tokens=int(usage_data.get("output_tokens", 0) or 0),
        )
        return TurnCompletedEvent(type=event_type, usage=usage)
    if event_type == "turn.failed":
        error = data.get("error") or {}
        return TurnFailedEvent(type=event_type, error=ThreadError(message=str(error.get("message", ""))))
    if event_type == "item.started":
        return ItemStartedEvent(type=event_type, item=parse_thread_item(data["item"]))
    if event_type == "item.updated":
        return ItemUpdatedEvent(type=event_type, item=parse_thread_item(data["item"]))
    if event_type == "item.completed":
        return ItemCompletedEvent(type=event_type, item=parse_thread_item(data["item"]))
    if event_type == "error":
        return ThreadErrorEvent(type=event_type, message=str(data.get("message", "")))
    raise ValueError(f"Unsupported event type: {event_type}")


def parse_thread_item(data: JsonDict) -> ThreadItem:
    item_type = data.get("type")
    item_id = str(data.get("id", ""))
    if item_type == "agent_message":
        return AgentMessageItem(id=item_id, type=item_type, text=str(data.get("text", "")))
    if item_type == "reasoning":
        return ReasoningItem(id=item_id, type=item_type, text=str(data.get("text", "")))
    if item_type == "command_execution":
        return CommandExecutionItem(
            id=item_id,
            type=item_type,
            command=str(data.get("command", "")),
            aggregated_output=str(data.get("aggregated_output", "")),
            exit_code=data.get("exit_code"),
            status=str(data.get("status", "")),
        )
    if item_type == "file_change":
        changes_data = data.get("changes") or []
        changes = [
            FileUpdateChange(path=str(change.get("path", "")), kind=str(change.get("kind", "")))
            for change in changes_data
        ]
        return FileChangeItem(id=item_id, type=item_type, changes=changes, status=str(data.get("status", "")))
    if item_type == "mcp_tool_call":
        result_data = data.get("result")
        error_data = data.get("error")
        result = None
        if isinstance(result_data, dict):
            result = McpToolCallResult(
                content=list(result_data.get("content") or []),
                structured_content=result_data.get("structured_content"),
            )
        error = None
        if isinstance(error_data, dict):
            error = McpToolCallError(message=str(error_data.get("message", "")))
        return McpToolCallItem(
            id=item_id,
            type=item_type,
            server=str(data.get("server", "")),
            tool=str(data.get("tool", "")),
            arguments=data.get("arguments"),
            status=str(data.get("status", "")),
            result=result,
            error=error,
        )
    if item_type == "web_search":
        return WebSearchItem(id=item_id, type=item_type, query=str(data.get("query", "")))
    if item_type == "todo_list":
        todos_data = data.get("items") or []
        todos = [
            TodoItem(text=str(todo.get("text", "")), completed=bool(todo.get("completed", False)))
            for todo in todos_data
        ]
        return TodoListItem(id=item_id, type=item_type, items=todos)
    if item_type == "error":
        return ErrorItem(id=item_id, type=item_type, message=str(data.get("message", "")))
    raise ValueError(f"Unsupported item type: {item_type}")
