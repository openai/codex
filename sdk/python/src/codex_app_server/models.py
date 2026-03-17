from __future__ import annotations

from dataclasses import dataclass
from typing import TypeAlias

from pydantic import BaseModel

from .generated.v2_all import (
    AccountLoginCompletedNotification,
    AccountRateLimitsUpdatedNotification,
    AccountUpdatedNotification,
    AgentMessageDeltaNotification,
    AppListUpdatedNotification,
    CommandExecutionOutputDeltaNotification,
    ConfigWarningNotification,
    ContextCompactedNotification,
    DeprecationNoticeNotification,
    ErrorNotification,
    FileChangeOutputDeltaNotification,
    ItemCompletedNotification,
    ItemStartedNotification,
    McpServerOauthLoginCompletedNotification,
    McpToolCallProgressNotification,
    PlanDeltaNotification,
    RawResponseItemCompletedNotification,
    ReasoningSummaryPartAddedNotification,
    ReasoningSummaryTextDeltaNotification,
    ReasoningTextDeltaNotification,
    TerminalInteractionNotification,
    ThreadNameUpdatedNotification,
    ThreadStartedNotification,
    ThreadTokenUsageUpdatedNotification,
    TurnCompletedNotification,
    TurnDiffUpdatedNotification,
    TurnPlanUpdatedNotification,
    TurnStartedNotification,
    WindowsWorldWritableWarningNotification,
)

JsonScalar: TypeAlias = str | int | float | bool | None
JsonValue: TypeAlias = JsonScalar | dict[str, "JsonValue"] | list["JsonValue"]
JsonObject: TypeAlias = dict[str, JsonValue]


@dataclass(slots=True)
class UnknownNotification:
    params: JsonObject


NotificationPayload: TypeAlias = (
    AccountLoginCompletedNotification
    | AccountRateLimitsUpdatedNotification
    | AccountUpdatedNotification
    | AgentMessageDeltaNotification
    | AppListUpdatedNotification
    | CommandExecutionOutputDeltaNotification
    | ConfigWarningNotification
    | ContextCompactedNotification
    | DeprecationNoticeNotification
    | ErrorNotification
    | FileChangeOutputDeltaNotification
    | ItemCompletedNotification
    | ItemStartedNotification
    | McpServerOauthLoginCompletedNotification
    | McpToolCallProgressNotification
    | PlanDeltaNotification
    | RawResponseItemCompletedNotification
    | ReasoningSummaryPartAddedNotification
    | ReasoningSummaryTextDeltaNotification
    | ReasoningTextDeltaNotification
    | TerminalInteractionNotification
    | ThreadNameUpdatedNotification
    | ThreadStartedNotification
    | ThreadTokenUsageUpdatedNotification
    | TurnCompletedNotification
    | TurnDiffUpdatedNotification
    | TurnPlanUpdatedNotification
    | TurnStartedNotification
    | WindowsWorldWritableWarningNotification
    | UnknownNotification
)


@dataclass(slots=True)
class Notification:
    method: str
    payload: NotificationPayload


class ServerInfo(BaseModel):
    name: str | None = None
    version: str | None = None


class InitializeResponse(BaseModel):
    serverInfo: ServerInfo | None = None
    userAgent: str | None = None
    platformFamily: str | None = None
    platformOs: str | None = None
