from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Iterator, Optional

from .config import CodexOptions, ThreadOptions, TurnOptions
from .events import (
    ItemCompletedEvent,
    ThreadErrorEvent,
    ThreadEvent,
    ThreadStartedEvent,
    TurnCompletedEvent,
    TurnFailedEvent,
    Usage,
    parse_thread_event,
)
from .exceptions import JsonParseError, ThreadRunError
from .exec import CodexExec, ExecArgs
from .items import AgentMessageItem, ThreadItem
from .schema import prepare_schema_file


@dataclass(frozen=True, slots=True)
class ThreadRunResult:
    items: list[ThreadItem]
    final_response: str
    usage: Optional[Usage]


@dataclass(frozen=True, slots=True)
class ThreadStream:
    events: Iterator[ThreadEvent]

    def __iter__(self) -> Iterator[ThreadEvent]:
        return self.events


class Thread:
    def __init__(
        self,
        exec_client: CodexExec,
        codex_options: CodexOptions,
        thread_options: ThreadOptions,
        thread_id: Optional[str] = None,
    ) -> None:
        self._exec = exec_client
        self._codex_options = codex_options
        self._thread_options = thread_options
        self._id = thread_id

    @property
    def id(self) -> Optional[str]:
        return self._id

    def run_streamed(self, prompt: str, turn_options: Optional[TurnOptions] = None) -> ThreadStream:
        events = self._stream_events(prompt, turn_options)
        return ThreadStream(events=events)

    def run(self, prompt: str, turn_options: Optional[TurnOptions] = None) -> ThreadRunResult:
        final_response = ""
        items: list[ThreadItem] = []
        usage: Optional[Usage] = None
        failure_message: Optional[str] = None

        for event in self._stream_events(prompt, turn_options):
            if isinstance(event, ThreadErrorEvent):
                raise ThreadRunError(event.message)
            if isinstance(event, TurnFailedEvent):
                failure_message = event.error.message
                break
            if isinstance(event, TurnCompletedEvent):
                usage = event.usage
            if isinstance(event, ItemCompletedEvent):
                item = event.item
                items.append(item)
                if isinstance(item, AgentMessageItem):
                    final_response = item.text

        if failure_message is not None:
            raise ThreadRunError(failure_message)

        return ThreadRunResult(items=items, final_response=final_response, usage=usage)

    def _stream_events(
        self,
        prompt: str,
        turn_options: Optional[TurnOptions],
    ) -> Iterator[ThreadEvent]:
        turn = turn_options or TurnOptions()
        with prepare_schema_file(turn.output_schema) as schema_file:
            exec_args = ExecArgs(
                input=prompt,
                base_url=self._codex_options.base_url,
                api_key=self._codex_options.api_key,
                thread_id=self._id,
                model=self._thread_options.model,
                sandbox_mode=self._thread_options.sandbox_mode,
                working_directory=self._thread_options.working_directory,
                skip_git_repo_check=self._thread_options.skip_git_repo_check,
                output_schema_path=str(schema_file.path) if schema_file.path else None,
            )
            command = tuple(self._exec.build_command(exec_args))
            for line in self._exec.run_lines(exec_args):
                try:
                    payload = json.loads(line)
                except json.JSONDecodeError as error:
                    raise JsonParseError(line, command) from error

                event = parse_thread_event(payload)
                if isinstance(event, ThreadStartedEvent):
                    self._id = event.thread_id
                yield event
