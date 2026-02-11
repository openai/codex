from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any, AsyncGenerator, Mapping

from .events import (
    ItemCompletedEvent,
    ThreadError,
    ThreadErrorEvent,
    ThreadEvent,
    ThreadStartedEvent,
    TurnCompletedEvent,
    TurnFailedEvent,
    Usage,
    parse_thread_event,
)
from .exec import CodexExec, CodexExecArgs
from .items import AgentMessageItem, ThreadItem
from .options import (
    CodexOptions,
    Input,
    ThreadOptions,
    TurnOptions,
    UserInput,
    coerce_turn_options,
)
from .output_schema_file import create_output_schema_file


@dataclass(slots=True)
class TurnResult:
    items: list[ThreadItem]
    final_response: str
    usage: Usage | None


@dataclass(slots=True)
class RunStreamedResult:
    events: AsyncGenerator[ThreadEvent, None]


@dataclass(slots=True)
class Thread:
    _exec: CodexExec
    _options: CodexOptions
    _thread_options: ThreadOptions
    _id: str | None = None

    @property
    def id(self) -> str | None:
        return self._id

    async def run_streamed(
        self,
        input: Input,
        turn_options: TurnOptions | Mapping[str, Any] | None = None,
    ) -> RunStreamedResult:
        return RunStreamedResult(events=self._run_streamed_internal(input, turn_options))

    async def _run_streamed_internal(
        self,
        input: Input,
        turn_options: TurnOptions | Mapping[str, Any] | None = None,
    ) -> AsyncGenerator[ThreadEvent, None]:
        options = coerce_turn_options(turn_options)
        schema_file = await create_output_schema_file(options.output_schema)
        try:
            prompt, images = normalize_input(input)
            exec_args = CodexExecArgs(
                input=prompt,
                base_url=self._options.base_url,
                api_key=self._options.api_key,
                thread_id=self._id,
                images=images,
                model=self._thread_options.model,
                sandbox_mode=self._thread_options.sandbox_mode,
                working_directory=self._thread_options.working_directory,
                additional_directories=self._thread_options.additional_directories,
                skip_git_repo_check=self._thread_options.skip_git_repo_check,
                output_schema_file=schema_file.schema_path,
                model_reasoning_effort=self._thread_options.model_reasoning_effort,
                signal=options.signal,
                network_access_enabled=self._thread_options.network_access_enabled,
                web_search_mode=self._thread_options.web_search_mode,
                web_search_enabled=self._thread_options.web_search_enabled,
                approval_policy=self._thread_options.approval_policy,
            )

            async for line in self._exec.run(exec_args):
                try:
                    raw_event = json.loads(line)
                except json.JSONDecodeError as error:
                    raise RuntimeError(f"Failed to parse event line: {line}") from error
                if not isinstance(raw_event, Mapping):
                    raise RuntimeError(f"Unexpected non-object event: {raw_event!r}")
                event = parse_thread_event(raw_event)
                if isinstance(event, ThreadStartedEvent):
                    self._id = event.thread_id
                yield event
        finally:
            await schema_file.cleanup()

    async def run(
        self,
        input: Input,
        turn_options: TurnOptions | Mapping[str, Any] | None = None,
    ) -> TurnResult:
        items: list[ThreadItem] = []
        final_response = ""
        usage: Usage | None = None
        turn_failure: ThreadError | None = None

        async for event in self._run_streamed_internal(input, turn_options):
            if isinstance(event, ItemCompletedEvent):
                if isinstance(event.item, AgentMessageItem):
                    final_response = event.item.text
                items.append(event.item)
            elif isinstance(event, TurnCompletedEvent):
                usage = event.usage
            elif isinstance(event, TurnFailedEvent):
                turn_failure = event.error
                break
            elif isinstance(event, ThreadErrorEvent):
                raise RuntimeError(event.message)

        if turn_failure is not None:
            raise RuntimeError(turn_failure.message)

        return TurnResult(items=items, final_response=final_response, usage=usage)


def normalize_input(input: Input) -> tuple[str, list[str]]:
    if isinstance(input, str):
        return input, []

    prompt_parts: list[str] = []
    images: list[str] = []
    for item in input:
        _normalize_input_item(item, prompt_parts, images)
    return "\n\n".join(prompt_parts), images


def _normalize_input_item(item: UserInput, prompt_parts: list[str], images: list[str]) -> None:
    item_type = item.get("type")
    if item_type == "text":
        prompt_parts.append(str(item.get("text", "")))
        return
    if item_type == "local_image":
        images.append(str(item.get("path", "")))
        return
    raise ValueError(f"Unknown user input type: {item_type}")
