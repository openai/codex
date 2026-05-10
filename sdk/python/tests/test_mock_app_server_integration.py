from __future__ import annotations

import asyncio
from collections.abc import AsyncIterator, Iterable, Iterator
from typing import Any

import pytest

from app_server_harness import (
    AppServerHarness,
    ev_assistant_message,
    ev_completed,
    ev_failed,
    ev_message_item_added,
    ev_output_text_delta,
    ev_response_created,
    sse,
)
from openai_codex import (
    ApprovalMode,
    AsyncCodex,
    Codex,
    ImageInput,
    LocalImageInput,
    TextInput,
)
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

TINY_PNG_BYTES = bytes(
    [
        137,
        80,
        78,
        71,
        13,
        10,
        26,
        10,
        0,
        0,
        0,
        13,
        73,
        72,
        68,
        82,
        0,
        0,
        0,
        1,
        0,
        0,
        0,
        1,
        8,
        6,
        0,
        0,
        0,
        31,
        21,
        196,
        137,
        0,
        0,
        0,
        11,
        73,
        68,
        65,
        84,
        120,
        156,
        99,
        96,
        0,
        2,
        0,
        0,
        5,
        0,
        1,
        122,
        94,
        171,
        63,
        0,
        0,
        0,
        0,
        73,
        69,
        78,
        68,
        174,
        66,
        96,
        130,
    ]
)


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


def _assistant_message_with_phase(
    item_id: str,
    text: str,
    phase: MessagePhase,
) -> dict[str, Any]:
    """Build an assistant message event carrying app-server phase metadata."""
    event = ev_assistant_message(item_id, text)
    event["item"] = {**event["item"], "phase": phase.value}
    return event


def _request_kind(request_path: str) -> str:
    """Classify captured mock-server request paths for compact assertions."""
    if request_path.endswith("/responses/compact"):
        return "compact"
    if request_path.endswith("/responses"):
        return "responses"
    return request_path


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


def test_run_result_item_semantics_use_real_app_server(tmp_path) -> None:
    """RunResult should reflect real item notifications, not synthetic client queues."""
    cases = [
        (
            "last unknown phase wins",
            sse(
                [
                    ev_response_created("items-last"),
                    ev_assistant_message("msg-items-first", "First message"),
                    ev_assistant_message("msg-items-second", "Second message"),
                    ev_completed("items-last"),
                ]
            ),
            "Second message",
            ["First message", "Second message"],
        ),
        (
            "empty last message is preserved",
            sse(
                [
                    ev_response_created("items-empty"),
                    ev_assistant_message("msg-items-nonempty", "First message"),
                    ev_assistant_message("msg-items-empty", ""),
                    ev_completed("items-empty"),
                ]
            ),
            "",
            ["First message", ""],
        ),
        (
            "commentary only is not final",
            sse(
                [
                    ev_response_created("items-commentary"),
                    _assistant_message_with_phase(
                        "msg-items-commentary",
                        "Commentary",
                        MessagePhase.commentary,
                    ),
                    ev_completed("items-commentary"),
                ]
            ),
            None,
            ["Commentary"],
        ),
    ]

    with AppServerHarness(tmp_path) as harness:
        for _, body, _, _ in cases:
            harness.responses.enqueue_sse(body)

        with Codex(config=harness.app_server_config()) as codex:
            results = [
                codex.thread_start().run(f"case: {name}") for name, _, _, _ in cases
            ]

    assert [
        {
            "final_response": result.final_response,
            "agent_messages": _agent_message_texts_from_items(result.items),
        }
        for result in results
    ] == [
        {
            "final_response": final_response,
            "agent_messages": agent_messages,
        }
        for _, _, final_response, agent_messages in cases
    ]


def test_thread_run_raises_when_real_app_server_reports_failed_turn(tmp_path) -> None:
    """Thread.run should surface the failed turn error emitted by app-server."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("failed-run"),
                    ev_failed("failed-run", "boom from mock model"),
                ]
            )
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            with pytest.raises(RuntimeError, match="boom from mock model"):
                thread.run("trigger failure")


def test_async_run_result_item_semantics_use_real_app_server(tmp_path) -> None:
    """Async RunResult should use the same real app-server notification mapping."""

    async def scenario() -> None:
        """Run multiple async result cases against one app-server process."""
        cases = [
            (
                "last async unknown phase wins",
                sse(
                    [
                        ev_response_created("async-items-last"),
                        ev_assistant_message(
                            "msg-async-items-first",
                            "First async message",
                        ),
                        ev_assistant_message(
                            "msg-async-items-second",
                            "Second async message",
                        ),
                        ev_completed("async-items-last"),
                    ]
                ),
                "Second async message",
                ["First async message", "Second async message"],
            ),
            (
                "async commentary only is not final",
                sse(
                    [
                        ev_response_created("async-items-commentary"),
                        _assistant_message_with_phase(
                            "msg-async-items-commentary",
                            "Async commentary",
                            MessagePhase.commentary,
                        ),
                        ev_completed("async-items-commentary"),
                    ]
                ),
                None,
                ["Async commentary"],
            ),
        ]

        with AppServerHarness(tmp_path) as harness:
            for _, body, _, _ in cases:
                harness.responses.enqueue_sse(body)

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                results = [
                    await (await codex.thread_start()).run(f"case: {name}")
                    for name, _, _, _ in cases
                ]

        assert [
            {
                "final_response": result.final_response,
                "agent_messages": _agent_message_texts_from_items(result.items),
            }
            for result in results
        ] == [
            {
                "final_response": final_response,
                "agent_messages": agent_messages,
            }
            for _, _, final_response, agent_messages in cases
        ]

    asyncio.run(scenario())


def test_multimodal_inputs_reach_responses_api_through_real_app_server(
    tmp_path,
) -> None:
    """Remote and local image inputs should survive the SDK and app-server boundary."""
    local_image = tmp_path / "local.png"
    local_image.write_bytes(TINY_PNG_BYTES)
    remote_image_url = "https://example.com/codex.png"

    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message(
            "remote image received",
            response_id="remote-image",
        )
        harness.responses.enqueue_assistant_message(
            "local image received",
            response_id="local-image",
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            remote_result = thread.run(
                [
                    TextInput("Describe the remote image."),
                    ImageInput(remote_image_url),
                ]
            )
            local_result = thread.run(
                [
                    TextInput("Describe the local image."),
                    LocalImageInput(str(local_image)),
                ]
            )
            requests = harness.responses.wait_for_requests(2)

    assert {
        "final_responses": [
            remote_result.final_response,
            local_result.final_response,
        ],
        "latest_user_texts": [
            request.message_input_texts("user")[-1] for request in requests
        ],
        "image_url_shapes": [
            requests[0].message_image_urls("user")[0],
            requests[1].message_image_urls("user")[-1].startswith(
                "data:image/png;base64,"
            ),
        ],
    } == {
        "final_responses": ["remote image received", "local image received"],
        "latest_user_texts": [
            "Describe the remote image.",
            "Describe the local image.",
        ],
        "image_url_shapes": [remote_image_url, True],
    }


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


def test_low_level_sync_stream_text_uses_real_turn_routing(tmp_path) -> None:
    """AppServerClient.stream_text should stream through a real app-server turn."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            _streaming_response("low-sync-stream", "msg-low-sync-stream", ["fir", "st"])
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
                _streaming_response(
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


def test_turn_steer_adds_follow_up_input_through_real_app_server(tmp_path) -> None:
    """Steering an active turn should create a follow-up Responses request."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            _streaming_response("steer-first", "msg-steer-first", ["before steer"]),
            delay_between_events_s=0.2,
        )
        harness.responses.enqueue_assistant_message(
            "after steer",
            response_id="steer-second",
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            turn = thread.turn(TextInput("Start a steerable turn."))
            harness.responses.wait_for_requests(1)
            steer = turn.steer(TextInput("Use this steering input."))
            events = list(turn.stream())
            requests = harness.responses.wait_for_requests(2)

    assert {
        "steered_turn_id": steer.turn_id,
        "turn_id": turn.id,
        "agent_messages": _agent_message_texts(events),
        "last_user_texts": [
            request.message_input_texts("user")[-1] for request in requests
        ],
    } == {
        "steered_turn_id": turn.id,
        "turn_id": turn.id,
        "agent_messages": ["before steer", "after steer"],
        "last_user_texts": [
            "Start a steerable turn.",
            "Use this steering input.",
        ],
    }


def test_turn_interrupt_stops_active_turn_and_follow_up_runs(tmp_path) -> None:
    """Interrupting an active turn should complete it and leave the thread usable."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            _streaming_response(
                "interrupt-first",
                "msg-interrupt-first",
                ["still ", "running"],
            ),
            delay_between_events_s=0.2,
        )
        harness.responses.enqueue_assistant_message(
            "after interrupt",
            response_id="interrupt-follow-up",
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            interrupted_turn = thread.turn(TextInput("Start a long turn."))
            harness.responses.wait_for_requests(1)
            interrupt_response = interrupted_turn.interrupt()
            completed = interrupted_turn.run()
            follow_up = thread.run("Continue after the interrupt.")

    assert {
        "interrupt_response": interrupt_response.model_dump(
            by_alias=True,
            mode="json",
        ),
        "interrupted_status": completed.status,
        "follow_up": follow_up.final_response,
    } == {
        "interrupt_response": {},
        "interrupted_status": TurnStatus.interrupted,
        "follow_up": "after interrupt",
    }


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


def test_approval_modes_preserve_real_app_server_state_without_override(
    tmp_path,
) -> None:
    """Resume, fork, and next turn should inherit approval settings unless overridden."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("source seeded", response_id="turn-mode-0")
        harness.responses.enqueue_assistant_message("turn override", response_id="turn-mode-1")
        harness.responses.enqueue_assistant_message("turn inherited", response_id="turn-mode-2")

        with Codex(config=harness.app_server_config()) as codex:
            source = codex.thread_start(approval_mode=ApprovalMode.deny_all)
            source_result = source.run("seed the source rollout")
            resumed = codex.thread_resume(source.id)
            forked = codex.thread_fork(source.id)
            explicit_fork = codex.thread_fork(
                source.id,
                approval_mode=ApprovalMode.auto_review,
            )

            turn_thread = codex.thread_start()
            first_result = turn_thread.run(
                "deny this and later turns",
                approval_mode=ApprovalMode.deny_all,
            )
            after_turn_override = codex._client.thread_resume(  # noqa: SLF001
                turn_thread.id,
                ThreadResumeParams(thread_id=turn_thread.id),
            )
            second_result = turn_thread.run("inherit previous approval mode")
            after_omitted_turn = codex._client.thread_resume(  # noqa: SLF001
                turn_thread.id,
                ThreadResumeParams(thread_id=turn_thread.id),
            )

            inherited_policies = {
                "resumed": _response_approval_policy(
                    codex._client.thread_resume(  # noqa: SLF001
                        resumed.id,
                        ThreadResumeParams(thread_id=resumed.id),
                    )
                ),
                "forked": _response_approval_policy(
                    codex._client.thread_resume(  # noqa: SLF001
                        forked.id,
                        ThreadResumeParams(thread_id=forked.id),
                    )
                ),
                "explicit_fork": _response_approval_policy(
                    codex._client.thread_resume(  # noqa: SLF001
                        explicit_fork.id,
                        ThreadResumeParams(thread_id=explicit_fork.id),
                    )
                ),
                "after_turn_override": _response_approval_policy(after_turn_override),
                "after_omitted_turn": _response_approval_policy(after_omitted_turn),
            }

    assert {
        "policies": inherited_policies,
        "final_responses": [
            source_result.final_response,
            first_result.final_response,
            second_result.final_response,
        ],
    } == {
        "policies": {
            "resumed": AskForApprovalValue.never.value,
            "forked": AskForApprovalValue.never.value,
            "explicit_fork": AskForApprovalValue.on_request.value,
            "after_turn_override": AskForApprovalValue.never.value,
            "after_omitted_turn": AskForApprovalValue.never.value,
        },
        "final_responses": ["source seeded", "turn override", "turn inherited"],
    }


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


def test_archive_unarchive_round_trip_uses_real_app_server(tmp_path) -> None:
    """Archive helpers should use real app-server lifecycle RPCs."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("materialized", response_id="archive-seed")

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            seeded = thread.run("materialize this thread before archive")
            archived = codex.thread_archive(thread.id)
            unarchived = codex.thread_unarchive(thread.id)
            read = unarchived.read()

    assert {
        "seeded_response": seeded.final_response,
        "archive_response": archived.model_dump(by_alias=True, mode="json"),
        "unarchived_id": unarchived.id,
        "read_id": read.thread.id,
    } == {
        "seeded_response": "materialized",
        "archive_response": {},
        "unarchived_id": thread.id,
        "read_id": thread.id,
    }


def test_models_and_compact_use_real_app_server_rpcs(tmp_path) -> None:
    """Model listing and compaction should go through real app-server methods."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("history", response_id="compact-history")
        harness.responses.enqueue_assistant_message(
            "compact summary",
            response_id="compact-summary",
        )

        with Codex(config=harness.app_server_config()) as codex:
            models = codex.models(include_hidden=True)
            thread = codex.thread_start()
            run_result = thread.run("create history")
            compact_response = thread.compact()
            requests = harness.responses.wait_for_requests(2)

    assert {
        "models_payload_has_data": isinstance(
            models.model_dump(by_alias=True, mode="json").get("data"),
            list,
        ),
        "run_final_response": run_result.final_response,
        "compact_response": compact_response.model_dump(
            by_alias=True,
            mode="json",
        ),
        "request_kinds": [_request_kind(request.path) for request in requests],
    } == {
        "models_payload_has_data": True,
        "run_final_response": "history",
        "compact_response": {},
        "request_kinds": ["responses", "responses"],
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
