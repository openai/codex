from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import Any, Literal, Mapping, Sequence, TypeAlias, TypedDict, Union, cast

ApprovalMode: TypeAlias = Literal["never", "on-request", "on-failure", "untrusted"]
SandboxMode: TypeAlias = Literal["read-only", "workspace-write", "danger-full-access"]
ModelReasoningEffort: TypeAlias = Literal["minimal", "low", "medium", "high", "xhigh"]
WebSearchMode: TypeAlias = Literal["disabled", "cached", "live"]


class TextInput(TypedDict):
    type: Literal["text"]
    text: str


class LocalImageInput(TypedDict):
    type: Literal["local_image"]
    path: str


UserInput: TypeAlias = Union[TextInput, LocalImageInput]
Input: TypeAlias = Union[str, Sequence[UserInput]]

CodexConfigValue: TypeAlias = Union[
    str, int, float, bool, list["CodexConfigValue"], "CodexConfigObject"
]
CodexConfigObject: TypeAlias = dict[str, CodexConfigValue]


@dataclass(slots=True)
class CodexOptions:
    codex_path_override: str | None = None
    base_url: str | None = None
    api_key: str | None = None
    config: CodexConfigObject | None = None
    env: dict[str, str] | None = None


@dataclass(slots=True)
class ThreadOptions:
    model: str | None = None
    sandbox_mode: SandboxMode | None = None
    working_directory: str | None = None
    skip_git_repo_check: bool = False
    model_reasoning_effort: ModelReasoningEffort | None = None
    network_access_enabled: bool | None = None
    web_search_mode: WebSearchMode | None = None
    web_search_enabled: bool | None = None
    approval_policy: ApprovalMode | None = None
    additional_directories: list[str] | None = None


@dataclass(slots=True)
class TurnOptions:
    output_schema: dict[str, Any] | None = None
    signal: asyncio.Event | None = None


def coerce_codex_options(options: CodexOptions | Mapping[str, Any] | None) -> CodexOptions:
    if options is None:
        return CodexOptions()
    if isinstance(options, CodexOptions):
        return options
    return CodexOptions(
        codex_path_override=cast(str | None, options.get("codex_path_override")),
        base_url=cast(str | None, options.get("base_url")),
        api_key=cast(str | None, options.get("api_key")),
        config=cast(CodexConfigObject | None, options.get("config")),
        env=cast(dict[str, str] | None, options.get("env")),
    )


def coerce_thread_options(options: ThreadOptions | Mapping[str, Any] | None) -> ThreadOptions:
    if options is None:
        return ThreadOptions()
    if isinstance(options, ThreadOptions):
        return options
    return ThreadOptions(
        model=cast(str | None, options.get("model")),
        sandbox_mode=cast(SandboxMode | None, options.get("sandbox_mode")),
        working_directory=cast(str | None, options.get("working_directory")),
        skip_git_repo_check=bool(options.get("skip_git_repo_check", False)),
        model_reasoning_effort=cast(
            ModelReasoningEffort | None, options.get("model_reasoning_effort")
        ),
        network_access_enabled=cast(bool | None, options.get("network_access_enabled")),
        web_search_mode=cast(WebSearchMode | None, options.get("web_search_mode")),
        web_search_enabled=cast(bool | None, options.get("web_search_enabled")),
        approval_policy=cast(ApprovalMode | None, options.get("approval_policy")),
        additional_directories=cast(
            list[str] | None,
            list(options.get("additional_directories", []))
            if options.get("additional_directories") is not None
            else None,
        ),
    )


def coerce_turn_options(options: TurnOptions | Mapping[str, Any] | None) -> TurnOptions:
    if options is None:
        return TurnOptions()
    if isinstance(options, TurnOptions):
        return options
    return TurnOptions(
        output_schema=cast(dict[str, Any] | None, options.get("output_schema")),
        signal=cast(asyncio.Event | None, options.get("signal")),
    )
