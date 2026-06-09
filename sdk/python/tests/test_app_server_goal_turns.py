import asyncio
import time

import pytest
from app_server_harness import (
    AppServerHarness,
    ev_assistant_message,
    ev_completed_with_usage,
    ev_failed,
    ev_function_call,
    ev_response_created,
    sse,
)
from app_server_helpers import (
    agent_message_texts,
    agent_message_texts_from_items,
    streaming_response,
)

from openai_codex import AsyncCodex, Codex
from openai_codex.errors import InvalidRequestError, TransportClosedError
from openai_codex.generated.notification_registry import notification_turn_id
from openai_codex.generated.v2_all import (
    AgentMessageDeltaNotification,
    IdleThreadStatus,
    ThreadGoalGetResponse,
    ThreadGoalStatus,
    TurnCompletedNotification,
    TurnStatus,
)


def _enqueue_completed_goal(
    harness: AppServerHarness,
    prefix: str,
    *,
    initial_text: str,
    final_text: str,
) -> None:
    harness.responses.enqueue_sse(
        sse(
            [
                ev_response_created(f"{prefix}-initial"),
                ev_assistant_message(f"msg-{prefix}-initial", initial_text),
                ev_completed_with_usage(
                    f"{prefix}-initial",
                    input_tokens=3,
                    cached_input_tokens=1,
                    output_tokens=2,
                    reasoning_output_tokens=1,
                    total_tokens=5,
                ),
            ]
        )
    )
    harness.responses.enqueue_sse(
        sse(
            [
                ev_response_created(f"{prefix}-complete-tool"),
                ev_function_call(
                    f"call-{prefix}-complete",
                    "update_goal",
                    '{"status":"complete"}',
                ),
                ev_completed_with_usage(
                    f"{prefix}-complete-tool",
                    input_tokens=4,
                    cached_input_tokens=1,
                    output_tokens=1,
                    reasoning_output_tokens=1,
                    total_tokens=5,
                ),
            ]
        )
    )
    harness.responses.enqueue_sse(
        sse(
            [
                ev_response_created(f"{prefix}-final"),
                ev_assistant_message(f"msg-{prefix}-final", final_text),
                ev_completed_with_usage(
                    f"{prefix}-final",
                    input_tokens=5,
                    cached_input_tokens=0,
                    output_tokens=3,
                    reasoning_output_tokens=0,
                    total_tokens=8,
                ),
            ]
        )
    )


def _enqueue_steerable_goal(
    harness: AppServerHarness,
    prefix: str,
    *,
    initial_text: str,
    continuation_chunks: list[str],
    final_text: str,
) -> None:
    harness.responses.enqueue_assistant_message(
        initial_text,
        response_id=f"{prefix}-initial",
    )
    harness.responses.enqueue_sse(
        streaming_response(
            f"{prefix}-continuation",
            f"msg-{prefix}-continuation",
            continuation_chunks,
        ),
        delay_between_events_s=0.15,
    )
    harness.responses.enqueue_sse(
        sse(
            [
                ev_response_created(f"{prefix}-complete-tool"),
                ev_function_call(
                    f"call-{prefix}-complete",
                    "update_goal",
                    '{"status":"complete"}',
                ),
                ev_completed_with_usage(
                    f"{prefix}-complete-tool",
                    input_tokens=1,
                    cached_input_tokens=0,
                    output_tokens=1,
                    reasoning_output_tokens=0,
                    total_tokens=2,
                ),
            ]
        )
    )
    harness.responses.enqueue_assistant_message(
        final_text,
        response_id=f"{prefix}-final",
    )


def _enqueue_interruptible_goal(
    harness: AppServerHarness,
    prefix: str,
    *,
    initial_text: str,
    continuation_chunks: list[str],
    follow_up_text: str,
) -> None:
    harness.responses.enqueue_assistant_message(
        initial_text,
        response_id=f"{prefix}-initial",
    )
    harness.responses.enqueue_sse(
        streaming_response(
            f"{prefix}-continuation",
            f"msg-{prefix}-continuation",
            continuation_chunks,
        ),
        delay_between_events_s=0.2,
    )
    harness.responses.enqueue_assistant_message(
        follow_up_text,
        response_id=f"{prefix}-follow-up",
    )


def _continuation_text(request) -> str:
    return "\n".join(request.message_input_texts("user"))


def test_sync_goal_run_aggregates_automatic_continuation(tmp_path) -> None:
    """The public result should cover the initial and automatic continuation turns."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        _enqueue_completed_goal(
            harness,
            "sync-run",
            initial_text="Initial pass complete.",
            final_text="Goal complete.",
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            result = thread.run_goal("  Improve benchmark coverage  ")
            requests = harness.responses.wait_for_requests(3)

    usage = result.usage.model_dump(by_alias=True, mode="json") if result.usage else None
    assert {
        "id_is_present": bool(result.id),
        "status": result.status,
        "messages": agent_message_texts_from_items(result.items),
        "final_response": result.final_response,
        "usage": usage,
        "timing": (
            result.started_at is not None,
            result.completed_at is not None,
            result.duration_ms == max(0, result.completed_at - result.started_at) * 1000
            if result.started_at is not None and result.completed_at is not None
            else False,
        ),
        "continuation_has_objective": "<objective>\nImprove benchmark coverage\n</objective>"
        in _continuation_text(requests[0]),
    } == {
        "id_is_present": True,
        "status": TurnStatus.completed,
        "messages": ["Initial pass complete.", "Goal complete."],
        "final_response": "Goal complete.",
        "usage": {
            "last": {
                "cachedInputTokens": 0,
                "inputTokens": 5,
                "outputTokens": 3,
                "reasoningOutputTokens": 0,
                "totalTokens": 8,
            },
            "modelContextWindow": 258_400,
            "total": {
                "cachedInputTokens": 2,
                "inputTokens": 12,
                "outputTokens": 6,
                "reasoningOutputTokens": 2,
                "totalTokens": 18,
            },
        },
        "timing": (True, True, True),
        "continuation_has_objective": True,
    }


def test_goal_stream_exposes_one_logical_lifecycle(tmp_path) -> None:
    """Continuation boundaries should not leak through the public stream."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        harness.responses.enqueue_sse(
            streaming_response(
                "stream-initial",
                "msg-stream-initial",
                ["initial ", "pass"],
            )
        )
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("stream-complete-tool"),
                    ev_function_call(
                        "call-stream-complete",
                        "update_goal",
                        '{"status":"complete"}',
                    ),
                    ev_completed_with_usage(
                        "stream-complete-tool",
                        input_tokens=1,
                        cached_input_tokens=0,
                        output_tokens=1,
                        reasoning_output_tokens=0,
                        total_tokens=2,
                    ),
                ]
            )
        )
        harness.responses.enqueue_sse(
            streaming_response(
                "stream-final",
                "msg-stream-final",
                ["goal ", "complete"],
            )
        )

        with Codex(config=harness.app_server_config()) as codex:
            turn = codex.thread_start().start_goal("Finish the integration suite")
            events = list(turn.stream())

    lifecycle = [event for event in events if event.method in {"turn/started", "turn/completed"}]
    routed_ids = [
        turn_id for event in events if (turn_id := notification_turn_id(event.payload)) is not None
    ]
    assert {
        "lifecycle": [event.method for event in lifecycle],
        "routed_ids": routed_ids,
        "deltas": [
            event.payload.delta
            for event in events
            if isinstance(event.payload, AgentMessageDeltaNotification)
        ],
        "messages": agent_message_texts(events),
        "completion_statuses": [
            event.payload.turn.status
            for event in events
            if isinstance(event.payload, TurnCompletedNotification)
        ],
    } == {
        "lifecycle": ["turn/started", "turn/completed"],
        "routed_ids": [turn.id] * len(routed_ids),
        "deltas": ["initial ", "pass", "goal ", "complete"],
        "messages": ["initial pass", "goal complete"],
        "completion_statuses": [TurnStatus.completed],
    }


def test_goal_can_complete_within_the_initial_server_turn(tmp_path) -> None:
    """Completing the goal before turn end should not create a continuation turn."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("single-turn-complete-tool"),
                    ev_function_call(
                        "call-single-turn-complete",
                        "update_goal",
                        '{"status":"complete"}',
                    ),
                    ev_completed_with_usage(
                        "single-turn-complete-tool",
                        input_tokens=1,
                        cached_input_tokens=0,
                        output_tokens=1,
                        reasoning_output_tokens=0,
                        total_tokens=2,
                    ),
                ]
            )
        )
        harness.responses.enqueue_assistant_message(
            "Completed without a continuation.",
            response_id="single-turn-final",
        )

        with Codex(config=harness.app_server_config()) as codex:
            turn = codex.thread_start().start_goal("Finish in the initial turn")
            result = turn.run()
            requests = harness.responses.wait_for_requests(2)
            with pytest.raises(InvalidRequestError) as steer_error:
                turn.steer("Keep working")
            with pytest.raises(InvalidRequestError) as interrupt_error:
                turn.interrupt()

    assert {
        "result": (result.id, result.status, result.final_response),
        "messages": agent_message_texts_from_items(result.items),
        "request_count": len(requests),
        "inactive_errors": [steer_error.value.message, interrupt_error.value.message],
    } == {
        "result": (turn.id, TurnStatus.completed, "Completed without a continuation."),
        "messages": ["Completed without a continuation."],
        "request_count": 2,
        "inactive_errors": ["no active turn to steer", "no active turn to interrupt"],
    }


def test_goal_replaces_an_existing_persisted_goal(tmp_path) -> None:
    """Goal mode should replace a resumable persisted goal before model work begins."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        _enqueue_completed_goal(
            harness,
            "replacement",
            initial_text="Replacement started.",
            final_text="Replacement complete.",
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            previous = codex._client.thread_goal_set(
                thread.id,
                objective="Keep the old benchmark objective",
                status=ThreadGoalStatus.paused,
            ).goal
            result = thread.run_goal("Publish the replacement objective")
            persisted = codex._client.request(
                "thread/goal/get",
                {"threadId": thread.id},
                response_model=ThreadGoalGetResponse,
            ).goal
            requests = harness.responses.wait_for_requests(3)

    assert {
        "previous": (previous.objective, previous.status),
        "result": (result.status, result.final_response),
        "persisted": (
            persisted.objective if persisted else None,
            persisted.status if persisted else None,
            persisted.token_budget if persisted else None,
        ),
        "continuation_has_replacement": (
            "Publish the replacement objective" in _continuation_text(requests[0])
        ),
        "continuation_has_previous": (
            "Keep the old benchmark objective" in _continuation_text(requests[0])
        ),
    } == {
        "previous": ("Keep the old benchmark objective", ThreadGoalStatus.paused),
        "result": (TurnStatus.completed, "Replacement complete."),
        "persisted": (
            "Publish the replacement objective",
            ThreadGoalStatus.complete,
            None,
        ),
        "continuation_has_replacement": True,
        "continuation_has_previous": False,
    }


def test_goal_steer_targets_an_active_continuation(tmp_path) -> None:
    """Steering should reach the active server turn while returning the logical ID."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        _enqueue_steerable_goal(
            harness,
            "steer",
            initial_text="Initial work complete.",
            continuation_chunks=["continuation ", "work"],
            final_text="Steered goal complete.",
        )

        with Codex(config=harness.app_server_config()) as codex:
            turn = codex.thread_start().start_goal("Start a goal that needs refinement")
            harness.responses.wait_for_requests(2)
            steer = turn.steer("Prioritize the edge-case coverage.")
            result = turn.run()
            requests = harness.responses.wait_for_requests(4)

    assert {
        "steer_id": steer.turn_id,
        "result": (result.id, result.status, result.final_response),
        "messages": agent_message_texts_from_items(result.items),
        "steering_input": requests[2].message_input_texts("user")[-1:],
    } == {
        "steer_id": turn.id,
        "result": (turn.id, TurnStatus.completed, "Steered goal complete."),
        "messages": [
            "Initial work complete.",
            "continuation work",
            "Steered goal complete.",
        ],
        "steering_input": ["Prioritize the edge-case coverage."],
    }


def test_goal_interrupt_pauses_continuation_and_leaves_thread_usable(tmp_path) -> None:
    """Interrupt should stop the logical goal operation and permit ordinary follow-up work."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        _enqueue_interruptible_goal(
            harness,
            "interrupt",
            initial_text="Initial work complete.",
            continuation_chunks=["still ", "working"],
            follow_up_text="Ordinary follow-up complete.",
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            turn = thread.start_goal("Start interruptible goal work")
            harness.responses.wait_for_requests(2)
            interrupt = turn.interrupt()
            interrupted = turn.run()
            follow_up = thread.run("Continue with an ordinary turn")
            harness.responses.wait_for_requests(3)
            requests = harness.responses.requests()

    assert {
        "interrupt": interrupt.model_dump(by_alias=True, mode="json"),
        "interrupted": (interrupted.id, interrupted.status),
        "follow_up": (follow_up.status, follow_up.final_response),
        "request_count": len(requests),
        "follow_up_input": requests[2].message_input_texts("user")[-1:],
    } == {
        "interrupt": {},
        "interrupted": (turn.id, TurnStatus.interrupted),
        "follow_up": (TurnStatus.completed, "Ordinary follow-up complete."),
        "request_count": 3,
        "follow_up_input": ["Continue with an ordinary turn"],
    }


def test_terminal_goal_failure_stops_continuation_and_releases_routing(tmp_path) -> None:
    """A failed server turn should stop rollover work and leave the thread usable."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("goal-terminal-failure"),
                    ev_failed("goal-terminal-failure", "goal model failed"),
                ]
            )
        )
        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            with pytest.raises(RuntimeError, match="goal model failed"):
                thread.run_goal("Fail this goal turn")

            harness.responses.enqueue_assistant_message(
                "Recovered with an ordinary turn.",
                response_id="goal-failure-follow-up",
            )
            follow_up = thread.run("Run after the goal failure")
            harness.responses.wait_for_requests(2)
            requests = harness.responses.requests()

    assert {
        "follow_up": (follow_up.status, follow_up.final_response),
        "request_count": len(requests),
        "follow_up_input": requests[1].message_input_texts("user")[-1:],
    } == {
        "follow_up": (TurnStatus.completed, "Recovered with an ordinary turn."),
        "request_count": 2,
        "follow_up_input": ["Run after the goal failure"],
    }


def test_closing_goal_stream_releases_real_process_routing(tmp_path) -> None:
    """Closing a stream should release SDK routing without pausing the persisted goal."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        harness.responses.enqueue_sse(
            streaming_response(
                "stream-close",
                "msg-stream-close",
                ["long ", "running ", "goal"],
            ),
            delay_between_events_s=0.5,
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            turn = thread.start_goal("Close this goal stream")
            harness.responses.wait_for_requests(1)
            stream = turn.stream()
            stream.close()
            registered_goals = dict(codex._client._router._goal_operations)
            persisted = codex._client.request(
                "thread/goal/get",
                {"threadId": thread.id},
                response_model=ThreadGoalGetResponse,
            ).goal

    assert {
        "registered_goals": registered_goals,
        "persisted_status": persisted.status if persisted else None,
    } == {
        "registered_goals": {},
        "persisted_status": ThreadGoalStatus.active,
    }


def test_app_server_exit_unblocks_goal_stream_and_releases_routing(tmp_path) -> None:
    """A real transport death should wake the logical reader and clean up routing."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        harness.responses.enqueue_sse(
            streaming_response(
                "transport-exit",
                "msg-transport-exit",
                ["long ", "running ", "goal"],
            ),
            delay_between_events_s=0.5,
        )

        with Codex(config=harness.app_server_config()) as codex:
            turn = codex.thread_start().start_goal("Stop the app-server during this goal")
            harness.responses.wait_for_requests(1)
            process = codex._client._proc
            assert process is not None
            process.terminate()
            process.wait(timeout=5)

            with pytest.raises(TransportClosedError):
                turn.run()
            registered_goals = dict(codex._client._router._goal_operations)

    assert registered_goals == {}


def test_failed_goal_starts_release_routing_without_model_requests(tmp_path) -> None:
    """Validation failures should leave the client and persisted thread usable."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        harness.responses.enqueue_assistant_message(
            "Ordinary turn complete.",
            response_id="validation-follow-up",
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            with pytest.raises(ValueError) as empty_error:
                thread.start_goal("   ")
            with pytest.raises(TypeError) as type_error:
                thread.start_goal(123)  # type: ignore[arg-type]
            with pytest.raises(ValueError) as long_error:
                thread.start_goal("x" * 4_001)

            ephemeral = codex.thread_start(ephemeral=True)
            with pytest.raises(InvalidRequestError) as ephemeral_error:
                ephemeral.start_goal("Persist this goal")

            follow_up = thread.run("Run after rejected goals")
            requests = harness.responses.wait_for_requests(1)
            registered_goals = dict(codex._client._router._goal_operations)

    assert {
        "errors": [
            str(empty_error.value),
            str(type_error.value),
            str(long_error.value),
            ephemeral_error.value.message,
        ],
        "follow_up": (follow_up.status, follow_up.final_response),
        "request_count": len(requests),
        "registered_goals": registered_goals,
    } == {
        "errors": [
            "goal objective must not be empty",
            "goal objective must be a string",
            "goal objective must be at most 4000 characters",
            f"thread must be persisted before starting a goal: {ephemeral.id}",
        ],
        "follow_up": (TurnStatus.completed, "Ordinary turn complete."),
        "request_count": 1,
        "registered_goals": {},
    }


def test_disabled_goals_fail_before_model_work_or_routing(tmp_path) -> None:
    """A runtime with goals disabled should reject startup without leaking state."""
    with AppServerHarness(tmp_path) as harness:
        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            with pytest.raises(InvalidRequestError) as error:
                thread.start_goal("This goal must not start")
            registered_goals = dict(codex._client._router._goal_operations)
            requests = harness.responses.requests()

    assert {
        "error": error.value.message,
        "registered_goals": registered_goals,
        "request_count": len(requests),
    } == {
        "error": "goals feature is disabled",
        "registered_goals": {},
        "request_count": 0,
    }


def test_active_thread_rejects_goal_start_and_keeps_ordinary_turn_usable(tmp_path) -> None:
    """Goal startup should require an idle thread without disturbing active work."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        harness.responses.enqueue_sse(
            streaming_response(
                "active-ordinary-turn",
                "msg-active-ordinary-turn",
                ["ordinary ", "work"],
            ),
            delay_between_events_s=0.5,
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            ordinary = thread.turn("Keep this ordinary turn active")
            harness.responses.wait_for_requests(1)
            with pytest.raises(InvalidRequestError) as error:
                thread.start_goal("Do not replace active work")
            ordinary.interrupt()
            result = ordinary.run()
            registered_goals = dict(codex._client._router._goal_operations)

    assert {
        "error": error.value.message,
        "ordinary_result": (result.id, result.status),
        "registered_goals": registered_goals,
    } == {
        "error": f"thread must be idle before starting a goal: {thread.id}",
        "ordinary_result": (ordinary.id, TurnStatus.interrupted),
        "registered_goals": {},
    }


def test_async_goal_run_matches_sync_logical_result(tmp_path) -> None:
    """The async public API should aggregate the same real continuation lifecycle."""

    async def scenario() -> None:
        with AppServerHarness(tmp_path, enable_goals=True) as harness:
            _enqueue_completed_goal(
                harness,
                "async-run",
                initial_text="Async initial pass.",
                final_text="Async goal complete.",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start()
                result = await thread.run_goal("Finish the async goal")
                requests = harness.responses.wait_for_requests(3)

        assert {
            "status": result.status,
            "messages": agent_message_texts_from_items(result.items),
            "final_response": result.final_response,
            "continuation_has_objective": (
                "Finish the async goal" in _continuation_text(requests[0])
            ),
        } == {
            "status": TurnStatus.completed,
            "messages": ["Async initial pass.", "Async goal complete."],
            "final_response": "Async goal complete.",
            "continuation_has_objective": True,
        }

    asyncio.run(scenario())


def test_async_goal_steer_targets_an_active_continuation(tmp_path) -> None:
    """Async steering should target the continuation and preserve the logical ID."""

    async def scenario() -> None:
        with AppServerHarness(tmp_path, enable_goals=True) as harness:
            _enqueue_steerable_goal(
                harness,
                "async-steer",
                initial_text="Async initial work complete.",
                continuation_chunks=["async continuation ", "work"],
                final_text="Async steered goal complete.",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start()
                turn = await thread.start_goal("Start an async goal that needs refinement")
                await asyncio.to_thread(harness.responses.wait_for_requests, 2)
                steer = await turn.steer("Prioritize async edge-case coverage.")
                result = await turn.run()
                requests = await asyncio.to_thread(harness.responses.wait_for_requests, 4)

        assert {
            "steer_id": steer.turn_id,
            "result": (result.id, result.status, result.final_response),
            "messages": agent_message_texts_from_items(result.items),
            "steering_input": requests[2].message_input_texts("user")[-1:],
        } == {
            "steer_id": turn.id,
            "result": (turn.id, TurnStatus.completed, "Async steered goal complete."),
            "messages": [
                "Async initial work complete.",
                "async continuation work",
                "Async steered goal complete.",
            ],
            "steering_input": ["Prioritize async edge-case coverage."],
        }

    asyncio.run(scenario())


def test_async_goal_interrupts_an_active_continuation(tmp_path) -> None:
    """Async interruption should stop the goal and leave the thread usable."""

    async def scenario() -> None:
        with AppServerHarness(tmp_path, enable_goals=True) as harness:
            _enqueue_interruptible_goal(
                harness,
                "async-interrupt",
                initial_text="Async initial work complete.",
                continuation_chunks=["async still ", "working"],
                follow_up_text="Async ordinary follow-up complete.",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start()
                turn = await thread.start_goal("Start async interruptible goal work")
                await asyncio.to_thread(harness.responses.wait_for_requests, 2)
                interrupt = await turn.interrupt()
                interrupted = await turn.run()
                follow_up = await thread.run("Continue with an async ordinary turn")
                await asyncio.to_thread(harness.responses.wait_for_requests, 3)
                requests = harness.responses.requests()

        assert {
            "interrupt": interrupt.model_dump(by_alias=True, mode="json"),
            "interrupted": (interrupted.id, interrupted.status),
            "follow_up": (follow_up.status, follow_up.final_response),
            "request_count": len(requests),
            "follow_up_input": requests[2].message_input_texts("user")[-1:],
        } == {
            "interrupt": {},
            "interrupted": (turn.id, TurnStatus.interrupted),
            "follow_up": (TurnStatus.completed, "Async ordinary follow-up complete."),
            "request_count": 3,
            "follow_up_input": ["Continue with an async ordinary turn"],
        }

    asyncio.run(scenario())


def test_async_goal_start_cancellation_interrupts_work_and_releases_routing(tmp_path) -> None:
    """Cancelling startup should pause and interrupt the physical goal turn."""

    async def scenario() -> None:
        with AppServerHarness(tmp_path, enable_goals=True) as harness:
            harness.responses.enqueue_sse(
                streaming_response(
                    "cancelled-start",
                    "msg-cancelled-start",
                    ["cancelled ", "goal ", "work"],
                ),
                delay_between_events_s=0.5,
            )
            harness.responses.enqueue_assistant_message(
                "Async follow-up complete.",
                response_id="cancelled-start-follow-up",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start()
                startup = asyncio.create_task(thread.start_goal("Cancel this goal during startup"))

                deadline = time.monotonic() + 5
                while not codex._client._sync._router._goal_operations:
                    if time.monotonic() >= deadline:
                        raise AssertionError("goal startup did not register routing")
                    await asyncio.sleep(0.01)

                # Keep the event loop occupied until model work starts so the
                # worker result cannot reach start_goal before cancellation.
                harness.responses.wait_for_requests(1)
                startup.cancel()
                with pytest.raises(asyncio.CancelledError):
                    await startup

                deadline = time.monotonic() + 5
                while True:
                    current = await thread.read()
                    registered_goals = dict(codex._client._sync._router._goal_operations)
                    if isinstance(current.thread.status.root, IdleThreadStatus) and not (
                        registered_goals
                    ):
                        break
                    if time.monotonic() >= deadline:
                        raise AssertionError("cancelled goal turn did not stop")
                    await asyncio.sleep(0.01)

                persisted = await codex._client.request(
                    "thread/goal/get",
                    {"threadId": thread.id},
                    response_model=ThreadGoalGetResponse,
                )
                follow_up = await thread.run("Continue after cancelled goal startup")
                requests = harness.responses.wait_for_requests(2)

        assert {
            "goal_status": persisted.goal.status if persisted.goal else None,
            "follow_up": (follow_up.status, follow_up.final_response),
            "request_count": len(requests),
            "registered_goals": registered_goals,
        } == {
            "goal_status": ThreadGoalStatus.paused,
            "follow_up": (TurnStatus.completed, "Async follow-up complete."),
            "request_count": 2,
            "registered_goals": {},
        }

    asyncio.run(scenario())
