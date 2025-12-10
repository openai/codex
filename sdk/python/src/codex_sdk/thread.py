from __future__ import annotations

from dataclasses import dataclass
from typing import Generator, List, Optional, Union

from .exec import CancelledError, CodexExec, CodexExecArgs
from .options import CodexOptions, ThreadOptions, TurnOptions
from .schema_file import create_output_schema_file
from .types import (
    AgentMessageItem,
    ItemCompletedEvent,
    ThreadErrorEvent,
    ThreadEvent,
    ThreadItem,
    ThreadStartedEvent,
    TurnCompletedEvent,
    TurnFailedEvent,
    Usage,
    parse_thread_event,
)

InputEntry = dict
Input = Union[str, List[InputEntry]]


@dataclass
class TurnResult:
    items: List[ThreadItem]
    final_response: str
    usage: Optional[Usage]


@dataclass
class StreamedTurn:
    events: Generator[ThreadEvent, None, None]


class ThreadRunError(Exception):
    """Raised when a turn fails."""


class Thread:
    def __init__(
        self,
        exec_client: CodexExec,
        options: CodexOptions,
        thread_options: ThreadOptions,
        thread_id: Optional[str] = None,
    ) -> None:
        self._exec = exec_client
        self._options = options
        self._thread_options = thread_options
        self._id = thread_id

    @property
    def id(self) -> Optional[str]:
        return self._id

    def run_streamed(self, input: Input, turn_options: Optional[TurnOptions] = None) -> StreamedTurn:
        return StreamedTurn(events=self._run_streamed_internal(input, turn_options or TurnOptions()))

    def _run_streamed_internal(
        self, input: Input, turn_options: TurnOptions
    ) -> Generator[ThreadEvent, None, None]:
        prompt, images = normalize_input(input)
        schema_path, cleanup = create_output_schema_file(turn_options.output_schema)
        args = CodexExecArgs(
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
            output_schema_file=schema_path,
            model_reasoning_effort=self._thread_options.model_reasoning_effort,
            cancellation_event=turn_options.cancellation_event,
            network_access_enabled=self._thread_options.network_access_enabled,
            web_search_enabled=self._thread_options.web_search_enabled,
            approval_policy=self._thread_options.approval_policy,
        )
        generator = self._exec.run(args)
        try:
            for line in generator:
                event = parse_thread_event(line)
                if isinstance(event, ThreadStartedEvent):
                    self._id = event.thread_id
                yield event
        finally:
            cleanup()

    def run(self, input: Input, turn_options: Optional[TurnOptions] = None) -> TurnResult:
        generator = self._run_streamed_internal(input, turn_options or TurnOptions())
        items: List[ThreadItem] = []
        final_response = ""
        usage: Optional[Usage] = None
        turn_failure: Optional[str] = None
        try:
            try:
                for event in generator:
                    if isinstance(event, ItemCompletedEvent):
                        if isinstance(event.item, AgentMessageItem):
                            final_response = event.item.text
                        items.append(event.item)
                    elif isinstance(event, TurnCompletedEvent):
                        usage = event.usage
                    elif isinstance(event, TurnFailedEvent):
                        turn_failure = event.error.message
                        break
                    elif isinstance(event, ThreadErrorEvent):
                        turn_failure = event.message
                        break
            except CancelledError:
                raise
        finally:
            generator.close()
        if turn_failure:
            raise ThreadRunError(turn_failure)
        return TurnResult(items=items, final_response=final_response, usage=usage)


def normalize_input(input: Input) -> tuple[str, List[str]]:
    if isinstance(input, str):
        return input, []
    prompt_parts: List[str] = []
    images: List[str] = []
    for item in input:
        item_type = item.get("type")
        if item_type == "text":
            text = item.get("text")
            if text is not None:
                prompt_parts.append(str(text))
        elif item_type == "local_image":
            path = item.get("path")
            if path:
                images.append(str(path))
    return "\n\n".join(prompt_parts), images
