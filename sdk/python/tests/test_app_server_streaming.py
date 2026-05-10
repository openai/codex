from __future__ import annotations

import asyncio

from app_server_harness import AppServerHarness
from openai_codex import AsyncCodex, Codex, TextInput
from openai_codex.generated.v2_all import (
    AgentMessageDeltaNotification,
    TurnCompletedNotification,
    TurnStatus,
)
from app_server_helpers import (
    agent_message_texts,
    next_async_delta,
    next_sync_delta,
    streaming_response,
)


def test_sync_stream_routes_text_deltas_and_completion(tmp_path) -> None:
    """A sync turn stream should expose deltas, completed items, and completion."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            streaming_response("stream-1", "msg-stream-1", ["hel", "lo"])
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            stream = thread.turn(TextInput("stream please")).stream()
            events = list(stream)

    assert {
        "deltas": [
            event.payload.delta
            for event in events
            if isinstance(event.payload, AgentMessageDeltaNotification)
        ],
        "agent_messages": agent_message_texts(events),
        "completed_statuses": [
            event.payload.turn.status
            for event in events
            if isinstance(event.payload, TurnCompletedNotification)
        ],
    } == {
        "deltas": ["hel", "lo"],
        "agent_messages": ["hello"],
        "completed_statuses": [TurnStatus.completed],
    }


def test_turn_run_returns_completed_turn(tmp_path) -> None:
    """TurnHandle.run should wait for the app-server completion notification."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("turn complete", response_id="turn-run-1")

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            turn = thread.turn(TextInput("complete this turn"))
            completed = turn.run()

    assert {
        "turn_id": completed.id,
        "status": completed.status,
        "items": completed.items,
    } == {
        "turn_id": turn.id,
        "status": TurnStatus.completed,
        "items": [],
    }


def test_async_stream_routes_text_deltas_and_completion(tmp_path) -> None:
    """An async turn stream should expose the same notification sequence."""

    async def scenario() -> None:
        """Stream one async turn against the real pinned app-server."""
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_sse(
                streaming_response("async-stream-1", "msg-async-stream-1", ["as", "ync"])
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start()
                turn = await thread.turn(TextInput("async stream please"))
                events = [event async for event in turn.stream()]

        assert {
            "deltas": [
                event.payload.delta
                for event in events
                if isinstance(event.payload, AgentMessageDeltaNotification)
            ],
            "agent_messages": agent_message_texts(events),
            "completed_statuses": [
                event.payload.turn.status
                for event in events
                if isinstance(event.payload, TurnCompletedNotification)
            ],
        } == {
            "deltas": ["as", "ync"],
            "agent_messages": ["async"],
            "completed_statuses": [TurnStatus.completed],
        }

    asyncio.run(scenario())


def test_low_level_sync_stream_text_uses_real_turn_routing(tmp_path) -> None:
    """AppServerClient.stream_text should stream through a real app-server turn."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            streaming_response("low-sync-stream", "msg-low-sync-stream", ["fir", "st"])
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            chunks = list(codex._client.stream_text(thread.id, "low-level sync"))  # noqa: SLF001

    assert [chunk.delta for chunk in chunks] == ["fir", "st"]


def test_low_level_async_stream_text_allows_parallel_model_list(tmp_path) -> None:
    """Async stream_text should yield without blocking another app-server request."""

    async def scenario() -> None:
        """Leave a stream open while another async request completes."""
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_sse(
                streaming_response(
                    "low-async-stream",
                    "msg-low-async-stream",
                    ["one", "two", "three"],
                ),
                delay_between_events_s=0.03,
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start()
                stream = codex._client.stream_text(  # noqa: SLF001
                    thread.id,
                    "low-level async",
                )
                first = await anext(stream)
                models_task = asyncio.create_task(codex.models())
                models = await asyncio.wait_for(models_task, timeout=1.0)
                remaining = [chunk.delta async for chunk in stream]

        assert {
            "first": first.delta,
            "remaining": remaining,
            "models_payload_has_data": isinstance(
                models.model_dump(by_alias=True, mode="json").get("data"),
                list,
            ),
        } == {
            "first": "one",
            "remaining": ["two", "three"],
            "models_payload_has_data": True,
        }

    asyncio.run(scenario())


def test_interleaved_sync_turn_streams_route_by_turn_id(tmp_path) -> None:
    """Two sync streams on one client should consume only their own notifications."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            streaming_response("first-stream", "msg-first", ["one-", "done"]),
            delay_between_events_s=0.01,
        )
        harness.responses.enqueue_sse(
            streaming_response("second-stream", "msg-second", ["two-", "done"]),
            delay_between_events_s=0.01,
        )

        with Codex(config=harness.app_server_config()) as codex:
            first_thread = codex.thread_start()
            second_thread = codex.thread_start()
            first_turn = first_thread.turn(TextInput("first"))
            second_turn = second_thread.turn(TextInput("second"))

            first_stream = first_turn.stream()
            second_stream = second_turn.stream()
            first_first_delta = next_sync_delta(first_stream)
            second_first_delta = next_sync_delta(second_stream)
            first_second_delta = next_sync_delta(first_stream)
            second_second_delta = next_sync_delta(second_stream)
            first_tail = list(first_stream)
            second_tail = list(second_stream)

    assert {
        "streams": sorted(
            [
                (
                    first_first_delta,
                    first_second_delta,
                    agent_message_texts(first_tail),
                ),
                (
                    second_first_delta,
                    second_second_delta,
                    agent_message_texts(second_tail),
                ),
            ]
        ),
    } == {
        "streams": [
            ("one-", "done", ["one-done"]),
            ("two-", "done", ["two-done"]),
        ],
    }


def test_interleaved_async_turn_streams_route_by_turn_id(tmp_path) -> None:
    """Two async streams on one client should consume only their own notifications."""

    async def scenario() -> None:
        """Interleave async stream consumers against one app-server process."""
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_sse(
                streaming_response("async-first", "msg-async-first", ["a1", "-done"]),
                delay_between_events_s=0.01,
            )
            harness.responses.enqueue_sse(
                streaming_response("async-second", "msg-async-second", ["a2", "-done"]),
                delay_between_events_s=0.01,
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                first_thread = await codex.thread_start()
                second_thread = await codex.thread_start()
                first_turn = await first_thread.turn(TextInput("async first"))
                second_turn = await second_thread.turn(TextInput("async second"))

                first_stream = first_turn.stream()
                second_stream = second_turn.stream()
                first_first_delta = await next_async_delta(first_stream)
                second_first_delta = await next_async_delta(second_stream)
                first_second_delta = await next_async_delta(first_stream)
                second_second_delta = await next_async_delta(second_stream)
                first_tail = [event async for event in first_stream]
                second_tail = [event async for event in second_stream]

        assert {
            "streams": sorted(
                [
                    (
                        first_first_delta,
                        first_second_delta,
                        agent_message_texts(first_tail),
                    ),
                    (
                        second_first_delta,
                        second_second_delta,
                        agent_message_texts(second_tail),
                    ),
                ]
            ),
        } == {
            "streams": [
                ("a1", "-done", ["a1-done"]),
                ("a2", "-done", ["a2-done"]),
            ],
        }

    asyncio.run(scenario())
