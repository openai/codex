from __future__ import annotations

from typing import Literal, TypedDict, NotRequired, Union

# Item types

CommandExecutionStatus = Literal["in_progress", "completed", "failed"]
PatchChangeKind = Literal["add", "delete", "update"]
PatchApplyStatus = Literal["completed", "failed"]
McpToolCallStatus = Literal["in_progress", "completed", "failed"]


class CommandExecutionItem(TypedDict):
    id: str
    type: Literal["command_execution"]
    command: str
    aggregated_output: str
    exit_code: NotRequired[int]
    status: CommandExecutionStatus


class FileUpdateChange(TypedDict):
    path: str
    kind: PatchChangeKind


class FileChangeItem(TypedDict):
    id: str
    type: Literal["file_change"]
    changes: list[FileUpdateChange]
    status: PatchApplyStatus


class McpToolCallResult(TypedDict):
    content: list[dict]
    structured_content: object


class McpToolCallError(TypedDict):
    message: str


class McpToolCallItem(TypedDict):
    id: str
    type: Literal["mcp_tool_call"]
    server: str
    tool: str
    arguments: object
    result: NotRequired[McpToolCallResult]
    error: NotRequired[McpToolCallError]
    status: McpToolCallStatus


class AgentMessageItem(TypedDict):
    id: str
    type: Literal["agent_message"]
    text: str


class ReasoningItem(TypedDict):
    id: str
    type: Literal["reasoning"]
    text: str


class WebSearchItem(TypedDict):
    id: str
    type: Literal["web_search"]
    query: str


class ErrorItem(TypedDict):
    id: str
    type: Literal["error"]
    message: str


class TodoItem(TypedDict):
    text: str
    completed: bool


class TodoListItem(TypedDict):
    id: str
    type: Literal["todo_list"]
    items: list[TodoItem]


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

# Events


class ThreadStartedEvent(TypedDict):
    type: Literal["thread.started"]
    thread_id: str


class TurnStartedEvent(TypedDict):
    type: Literal["turn.started"]


class Usage(TypedDict):
    input_tokens: int
    cached_input_tokens: int
    output_tokens: int


class TurnCompletedEvent(TypedDict):
    type: Literal["turn.completed"]
    usage: Usage


class ThreadError(TypedDict):
    message: str


class TurnFailedEvent(TypedDict):
    type: Literal["turn.failed"]
    error: ThreadError


class ItemStartedEvent(TypedDict):
    type: Literal["item.started"]
    item: ThreadItem


class ItemUpdatedEvent(TypedDict):
    type: Literal["item.updated"]
    item: ThreadItem


class ItemCompletedEvent(TypedDict):
    type: Literal["item.completed"]
    item: ThreadItem


class ThreadErrorEvent(TypedDict):
    type: Literal["error"]
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

# Inputs


class TextInput(TypedDict):
    type: Literal["text"]
    text: str


class LocalImageInput(TypedDict):
    type: Literal["local_image"]
    path: str


UserInput = Union[TextInput, LocalImageInput]
Input = Union[str, list[UserInput]]
