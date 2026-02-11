from __future__ import annotations

import json
from pathlib import Path

import pytest

from openai_codex_sdk.events import ItemCompletedEvent, ThreadStartedEvent, TurnCompletedEvent
from openai_codex_sdk.options import CodexOptions, ThreadOptions, TurnOptions
from openai_codex_sdk.thread import Thread


class FakeExec:
    def __init__(self, event_batches: list[list[dict]]) -> None:
        self._event_batches = event_batches
        self.calls = []

    async def run(self, args):  # noqa: ANN001
        self.calls.append(args)
        if not self._event_batches:
            return
        for event in self._event_batches.pop(0):
            yield json.dumps(event)


@pytest.mark.asyncio
async def test_run_returns_completed_turn() -> None:
    fake_exec = FakeExec(
        [
            [
                {"type": "thread.started", "thread_id": "thread_1"},
                {"type": "turn.started"},
                {
                    "type": "item.completed",
                    "item": {"id": "item_1", "type": "agent_message", "text": "Hi!"},
                },
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 42,
                        "cached_input_tokens": 12,
                        "output_tokens": 5,
                    },
                },
            ]
        ]
    )
    thread = Thread(
        _exec=fake_exec,
        _options=CodexOptions(base_url="http://localhost", api_key="test"),
        _thread_options=ThreadOptions(),
    )

    result = await thread.run("Hello")

    assert thread.id == "thread_1"
    assert result.final_response == "Hi!"
    assert result.usage is not None
    assert result.usage.input_tokens == 42
    assert len(result.items) == 1


@pytest.mark.asyncio
async def test_run_streamed_exposes_events_and_sets_thread_id() -> None:
    fake_exec = FakeExec(
        [
            [
                {"type": "thread.started", "thread_id": "thread_1"},
                {"type": "turn.started"},
                {
                    "type": "item.completed",
                    "item": {"id": "item_1", "type": "agent_message", "text": "Hi!"},
                },
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 1,
                        "cached_input_tokens": 0,
                        "output_tokens": 1,
                    },
                },
            ]
        ]
    )
    thread = Thread(
        _exec=fake_exec,
        _options=CodexOptions(),
        _thread_options=ThreadOptions(),
    )

    streamed = await thread.run_streamed("hello")
    events = [event async for event in streamed.events]

    assert any(isinstance(event, ThreadStartedEvent) for event in events)
    assert any(isinstance(event, ItemCompletedEvent) for event in events)
    assert any(isinstance(event, TurnCompletedEvent) for event in events)
    assert thread.id == "thread_1"


@pytest.mark.asyncio
async def test_second_run_uses_existing_thread_and_images() -> None:
    fake_exec = FakeExec(
        [
            [
                {"type": "thread.started", "thread_id": "thread_1"},
                {"type": "turn.started"},
                {
                    "type": "item.completed",
                    "item": {"id": "item_1", "type": "agent_message", "text": "First"},
                },
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 1,
                        "cached_input_tokens": 0,
                        "output_tokens": 1,
                    },
                },
            ],
            [
                {"type": "turn.started"},
                {
                    "type": "item.completed",
                    "item": {"id": "item_2", "type": "agent_message", "text": "Second"},
                },
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 1,
                        "cached_input_tokens": 0,
                        "output_tokens": 1,
                    },
                },
            ],
        ]
    )
    thread = Thread(
        _exec=fake_exec,
        _options=CodexOptions(),
        _thread_options=ThreadOptions(),
    )

    await thread.run("first input")
    await thread.run(
        [
            {"type": "text", "text": "Describe file changes"},
            {"type": "text", "text": "Focus on impacted tests"},
            {"type": "local_image", "path": "/tmp/a.png"},
        ]
    )

    first_call = fake_exec.calls[0]
    second_call = fake_exec.calls[1]
    assert first_call.thread_id is None
    assert second_call.thread_id == "thread_1"
    assert second_call.input == "Describe file changes\n\nFocus on impacted tests"
    assert second_call.images == ["/tmp/a.png"]


@pytest.mark.asyncio
async def test_output_schema_file_is_cleaned_after_run() -> None:
    schema_paths: list[str] = []

    class SchemaInspectingExec(FakeExec):
        async def run(self, args):  # noqa: ANN001
            if args.output_schema_file is not None:
                schema_paths.append(args.output_schema_file)
                assert Path(args.output_schema_file).exists()
            async for line in super().run(args):
                yield line

    fake_exec = SchemaInspectingExec(
        [
            [
                {"type": "thread.started", "thread_id": "thread_1"},
                {"type": "turn.started"},
                {
                    "type": "item.completed",
                    "item": {"id": "item_1", "type": "agent_message", "text": "ok"},
                },
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 1,
                        "cached_input_tokens": 0,
                        "output_tokens": 1,
                    },
                },
            ]
        ]
    )
    thread = Thread(
        _exec=fake_exec,
        _options=CodexOptions(),
        _thread_options=ThreadOptions(),
    )

    schema = {"type": "object", "properties": {"summary": {"type": "string"}}}
    await thread.run("structured", TurnOptions(output_schema=schema))

    assert len(schema_paths) == 1
    assert not Path(schema_paths[0]).exists()


@pytest.mark.asyncio
async def test_turn_failed_raises() -> None:
    fake_exec = FakeExec(
        [
            [
                {"type": "thread.started", "thread_id": "thread_1"},
                {"type": "turn.started"},
                {"type": "turn.failed", "error": {"message": "rate limit exceeded"}},
            ]
        ]
    )
    thread = Thread(
        _exec=fake_exec,
        _options=CodexOptions(),
        _thread_options=ThreadOptions(),
    )

    with pytest.raises(RuntimeError, match="rate limit exceeded"):
        await thread.run("fail")
