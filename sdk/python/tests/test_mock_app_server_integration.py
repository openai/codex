from __future__ import annotations

import asyncio
from collections.abc import AsyncIterator, Iterable, Iterator
from typing import Any

from app_server_harness import (
    AppServerHarness,
    ev_assistant_message,
    ev_completed,
    ev_message_item_added,
    ev_output_text_delta,
    ev_response_created,
    sse,
)
from openai_codex import ApprovalMode, AsyncCodex, Codex, TextInput
from openai_codex.generated.v2_all import (
    AgentMessageDeltaNotification,
    AskForApprovalValue,
    ItemCompletedNotification,
    MessagePhase,
    ThreadResumeParams,
    TurnCompletedNotification,
    TurnStatus,
)
from openai_codex.models import Notification


def _response_approval_policy(response: Any) -> str:
    """Return serialized approvalPolicy from a generated thread response."""
    return response.model_dump(by_alias=True, mode="json")["approvalPolicy"]


def _agent_message_texts(events: list[Notification]) -> list[str]:
    """Extract completed agent-message text from SDK notifications."""
    texts: list[str] = []
    for event in events:
        if not isinstance(event.payload, ItemCompletedNotification):
            continue
        item = event.payload.item.root
        if item.type == "agentMessage":
            texts.append(item.text)
    return texts


def _agent_message_texts_from_items(items: Iterable[Any]) -> list[str]:
    """Extract agent-message text from completed run result items."""
    texts: list[str] = []
    for item in items:
        root = item.root
        if root.type == "agentMessage":
            texts.append(root.text)
    return texts


def _next_sync_delta(stream: Iterator[Notification]) -> str:
    """Advance a sync turn stream until the next agent-message text delta."""
    for event in stream:
        if isinstance(event.payload, AgentMessageDeltaNotification):
            return event.payload.delta
    raise AssertionError("stream completed before an agent-message delta")


async def _next_async_delta(stream: AsyncIterator[Notification]) -> str:
    """Advance an async turn stream until the next agent-message text delta."""
    async for event in stream:
        if isinstance(event.payload, AgentMessageDeltaNotification):
            return event.payload.delta
    raise AssertionError("stream completed before an agent-message delta")


def _streaming_response(response_id: str, item_id: str, parts: list[str]) -> str:
    """Build an SSE stream with text deltas and a final assistant message."""
    return sse(
        [
            ev_response_created(response_id),
            ev_message_item_added(item_id),
            *[ev_output_text_delta(part) for part in parts],
            ev_assistant_message(item_id, "".join(parts)),
            ev_completed(response_id),
        ]
    )


def test_sync_thread_run_uses_pinned_app_server_and_mock_responses(
    tmp_path,
) -> None:
    """Drive Thread.run through the pinned app-server and inspect the HTTP request."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("Hello from the mock.", response_id="run-1")

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            result = thread.run("hello")

        request = harness.responses.single_request()

    body = request.body_json()
    assert {
        "final_response": result.final_response,
        "agent_messages": _agent_message_texts_from_items(result.items),
        "has_usage": result.usage is not None,
        "request_model": body["model"],
        "request_stream": body["stream"],
        "request_user_texts": request.message_input_texts("user")[-1:],
    } == {
        "final_response": "Hello from the mock.",
        "agent_messages": ["Hello from the mock."],
        "has_usage": True,
        "request_model": "mock-model",
        "request_stream": True,
        "request_user_texts": ["hello"],
    }


def test_async_thread_run_uses_pinned_app_server_and_mock_responses(
    tmp_path,
) -> None:
    """Async Thread.run should exercise the same app-server boundary."""

    async def scenario() -> None:
        """Run the async client against a real app-server process."""
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_assistant_message(
                "Hello async.",
                response_id="async-run-1",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start()
                result = await thread.run("async hello")

            request = harness.responses.single_request()

        assert {
            "final_response": result.final_response,
            "agent_messages": _agent_message_texts_from_items(result.items),
            "request_user_texts": request.message_input_texts("user")[-1:],
        } == {
            "final_response": "Hello async.",
            "agent_messages": ["Hello async."],
            "request_user_texts": ["async hello"],
        }

    asyncio.run(scenario())


def test_sync_stream_routes_text_deltas_and_completion(tmp_path) -> None:
    """A sync turn stream should expose deltas, completed items, and completion."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            _streaming_response("stream-1", "msg-stream-1", ["hel", "lo"])
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
        "agent_messages": _agent_message_texts(events),
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


def test_turn_run_returns_completed_turn_from_real_app_server(tmp_path) -> None:
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
                _streaming_response("async-stream-1", "msg-async-stream-1", ["as", "ync"])
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
            "agent_messages": _agent_message_texts(events),
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


def test_interleaved_sync_turn_streams_route_by_turn_id(tmp_path) -> None:
    """Two sync streams on one client should consume only their own notifications."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            _streaming_response("first-stream", "msg-first", ["one-", "done"]),
            delay_between_events_s=0.01,
        )
        harness.responses.enqueue_sse(
            _streaming_response("second-stream", "msg-second", ["two-", "done"]),
            delay_between_events_s=0.01,
        )

        with Codex(config=harness.app_server_config()) as codex:
            first_thread = codex.thread_start()
            second_thread = codex.thread_start()
            first_turn = first_thread.turn(TextInput("first"))
            second_turn = second_thread.turn(TextInput("second"))

            first_stream = first_turn.stream()
            second_stream = second_turn.stream()
            first_first_delta = _next_sync_delta(first_stream)
            second_first_delta = _next_sync_delta(second_stream)
            first_second_delta = _next_sync_delta(first_stream)
            second_second_delta = _next_sync_delta(second_stream)
            first_tail = list(first_stream)
            second_tail = list(second_stream)

    assert {
        "streams": sorted(
            [
                (
                    first_first_delta,
                    first_second_delta,
                    _agent_message_texts(first_tail),
                ),
                (
                    second_first_delta,
                    second_second_delta,
                    _agent_message_texts(second_tail),
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
                _streaming_response("async-first", "msg-async-first", ["a1", "-done"]),
                delay_between_events_s=0.01,
            )
            harness.responses.enqueue_sse(
                _streaming_response("async-second", "msg-async-second", ["a2", "-done"]),
                delay_between_events_s=0.01,
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                first_thread = await codex.thread_start()
                second_thread = await codex.thread_start()
                first_turn = await first_thread.turn(TextInput("async first"))
                second_turn = await second_thread.turn(TextInput("async second"))

                first_stream = first_turn.stream()
                second_stream = second_turn.stream()
                first_first_delta = await _next_async_delta(first_stream)
                second_first_delta = await _next_async_delta(second_stream)
                first_second_delta = await _next_async_delta(first_stream)
                second_second_delta = await _next_async_delta(second_stream)
                first_tail = [event async for event in first_stream]
                second_tail = [event async for event in second_stream]

        assert {
            "streams": sorted(
                [
                    (
                        first_first_delta,
                        first_second_delta,
                        _agent_message_texts(first_tail),
                    ),
                    (
                        second_first_delta,
                        second_second_delta,
                        _agent_message_texts(second_tail),
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


def test_thread_run_approval_mode_persists_until_explicit_override(tmp_path) -> None:
    """Omitted run approval mode should not rewrite the thread's stored setting."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("locked down", response_id="approval-1")
        harness.responses.enqueue_assistant_message("reviewable", response_id="approval-2")

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start(approval_mode=ApprovalMode.deny_all)

            first_result = thread.run("keep approvals denied")
            after_default_run = codex._client.thread_resume(  # noqa: SLF001
                thread.id,
                ThreadResumeParams(thread_id=thread.id),
            )
            second_result = thread.run(
                "allow auto review now",
                approval_mode=ApprovalMode.auto_review,
            )
            after_override_run = codex._client.thread_resume(  # noqa: SLF001
                thread.id,
                ThreadResumeParams(thread_id=thread.id),
            )

    assert {
        "after_default_policy": _response_approval_policy(after_default_run),
        "after_override_policy": _response_approval_policy(after_override_run),
        "final_responses": [
            first_result.final_response,
            second_result.final_response,
        ],
    } == {
        "after_default_policy": AskForApprovalValue.never.value,
        "after_override_policy": AskForApprovalValue.on_request.value,
        "final_responses": ["locked down", "reviewable"],
    }


def test_async_thread_run_approval_mode_persists_until_explicit_override(
    tmp_path,
) -> None:
    """Async omitted run approval mode should leave stored settings alone."""

    async def scenario() -> None:
        """Use the async client to verify persisted app-server approval state."""
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_assistant_message(
                "async locked down",
                response_id="async-approval-1",
            )
            harness.responses.enqueue_assistant_message(
                "async reviewable",
                response_id="async-approval-2",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start(approval_mode=ApprovalMode.deny_all)
                first_result = await thread.run("keep async approvals denied")
                after_default_run = await codex._client.thread_resume(  # noqa: SLF001
                    thread.id,
                    ThreadResumeParams(thread_id=thread.id),
                )
                second_result = await thread.run(
                    "allow async auto review now",
                    approval_mode=ApprovalMode.auto_review,
                )
                after_override_run = await codex._client.thread_resume(  # noqa: SLF001
                    thread.id,
                    ThreadResumeParams(thread_id=thread.id),
                )

        assert {
            "after_default_policy": _response_approval_policy(after_default_run),
            "after_override_policy": _response_approval_policy(after_override_run),
            "final_responses": [
                first_result.final_response,
                second_result.final_response,
            ],
        } == {
            "after_default_policy": AskForApprovalValue.never.value,
            "after_override_policy": AskForApprovalValue.on_request.value,
            "final_responses": ["async locked down", "async reviewable"],
        }

    asyncio.run(scenario())


def test_thread_lifecycle_uses_real_app_server_without_model_mocking(tmp_path) -> None:
    """Thread lifecycle helpers should operate through app-server JSON-RPC."""
    with AppServerHarness(tmp_path) as harness:
        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            thread.set_name("sdk integration thread")
            named = thread.read(include_turns=True)
            forked = codex.thread_fork(thread.id)

    assert {
        "name": named.thread.name,
        "fork_parent": forked.id != thread.id,
    } == {
        "name": "sdk integration thread",
        "fork_parent": True,
    }


def test_final_answer_phase_survives_real_app_server_mapping(tmp_path) -> None:
    """RunResult should use the final-answer item emitted by app-server."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("phase-1"),
                    {
                        **ev_assistant_message("msg-commentary", "Commentary"),
                        "item": {
                            **ev_assistant_message("msg-commentary", "Commentary")["item"],
                            "phase": MessagePhase.commentary.value,
                        },
                    },
                    {
                        **ev_assistant_message("msg-final", "Final answer"),
                        "item": {
                            **ev_assistant_message("msg-final", "Final answer")["item"],
                            "phase": MessagePhase.final_answer.value,
                        },
                    },
                    ev_completed("phase-1"),
                ]
            )
        )

        with Codex(config=harness.app_server_config()) as codex:
            result = codex.thread_start().run("choose final answer")

    assert {
        "final_response": result.final_response,
        "items": [
            {
                "text": item.root.text,
                "phase": None if item.root.phase is None else item.root.phase.value,
            }
            for item in result.items
            if item.root.type == "agentMessage"
        ],
    } == {
        "final_response": "Final answer",
        "items": [
            {"text": "Commentary", "phase": MessagePhase.commentary.value},
            {"text": "Final answer", "phase": MessagePhase.final_answer.value},
        ],
    }
