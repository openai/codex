from __future__ import annotations

import json
from dataclasses import dataclass
from typing import AsyncIterator

from ..errors import ThreadRunError
from ..exec import CodexExecArgs
from ..options import CodexOptions, ThreadOptions, TurnOptions
from ..schema import create_output_schema_file
from ..thread import _ensure_git_repo, _normalize_input
from ..types import Input, ThreadEvent, ThreadItem, ThreadError, Usage, UserInput
from .exec import AsyncCodexExec


@dataclass
class Turn:
    items: list[ThreadItem]
    final_response: str
    usage: Usage | None


RunResult = Turn


@dataclass
class StreamedTurn:
    events: AsyncIterator[ThreadEvent]


RunStreamedResult = StreamedTurn


class AsyncThread:
    def __init__(
        self,
        exec_client: AsyncCodexExec,
        options: CodexOptions,
        thread_options: ThreadOptions,
        thread_id: str | None = None,
    ) -> None:
        self._exec = exec_client
        self._options = options
        self._thread_options = thread_options
        self._id = thread_id

    @property
    def id(self) -> str | None:
        return self._id

    async def run_streamed(
        self, input: Input, turn_options: TurnOptions | None = None, **kwargs: object
    ) -> StreamedTurn:
        if turn_options is not None and kwargs:
            raise ValueError("Provide either TurnOptions or keyword arguments, not both")
        options = turn_options or TurnOptions(**kwargs)
        return StreamedTurn(events=self._run_streamed_internal(input, options))

    async def _run_streamed_internal(
        self, input: Input, turn_options: TurnOptions
    ) -> AsyncIterator[ThreadEvent]:
        schema_file = create_output_schema_file(turn_options.output_schema)
        options = self._thread_options
        prompt, images = _normalize_input(input)

        if options.working_directory and not options.skip_git_repo_check:
            _ensure_git_repo(options.working_directory)

        args = CodexExecArgs(
            input=prompt,
            base_url=self._options.base_url,
            api_key=self._options.api_key,
            thread_id=self._id,
            images=images,
            model=options.model,
            sandbox_mode=options.sandbox_mode,
            working_directory=options.working_directory,
            skip_git_repo_check=options.skip_git_repo_check,
            output_schema_file=schema_file.schema_path,
            model_reasoning_effort=options.model_reasoning_effort,
            signal=turn_options.signal,
            network_access_enabled=options.network_access_enabled,
            web_search_enabled=options.web_search_enabled,
            approval_policy=options.approval_policy,
            additional_directories=options.additional_directories or None,
        )

        try:
            async for line in self._exec.run(args):
                try:
                    event = json.loads(line)
                except json.JSONDecodeError as exc:
                    raise ThreadRunError(f"Failed to parse item: {line}") from exc

                if event.get("type") == "thread.started":
                    self._id = event.get("thread_id")
                yield event
        finally:
            schema_file.cleanup()

    async def run(
        self, input: Input, turn_options: TurnOptions | None = None, **kwargs: object
    ) -> Turn:
        if turn_options is not None and kwargs:
            raise ValueError("Provide either TurnOptions or keyword arguments, not both")
        turn_options = turn_options or TurnOptions(**kwargs)
        items: list[ThreadItem] = []
        final_response = ""
        usage: Usage | None = None
        turn_failure: ThreadError | None = None

        async for event in self._run_streamed_internal(input, turn_options):
            event_type = event.get("type")
            if event_type == "item.completed":
                item = event.get("item")
                if item and item.get("type") == "agent_message":
                    final_response = item.get("text", "")
                if item:
                    items.append(item)
            elif event_type == "turn.completed":
                usage = event.get("usage")
            elif event_type == "turn.failed":
                turn_failure = event.get("error")
                break

        if turn_failure:
            raise ThreadRunError(turn_failure.get("message", "Turn failed"))

        return Turn(items=items, final_response=final_response, usage=usage)
