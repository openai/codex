import asyncio
import os

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

from openai_codex import AsyncCodex, Codex, CodexConfig, TextInput
from openai_codex.errors import InvalidRequestError, TransportClosedError
from openai_codex.generated.notification_registry import notification_turn_id
from openai_codex.generated.v2_all import (
    AgentMessageDeltaNotification,
    ThreadGoalGetResponse,
    ThreadGoalSetResponse,
    ThreadGoalStatus,
    TurnCompletedNotification,
    TurnStatus,
)

SOURCE_CODEX_BIN = os.environ.get("CODEX_EXEC_PATH")

pytestmark = pytest.mark.skipif(
    SOURCE_CODEX_BIN is None,
    reason="requires CODEX_EXEC_PATH pointing to the checkout-built Codex binary",
)


def _source_config(harness: AppServerHarness) -> CodexConfig:
    assert SOURCE_CODEX_BIN is not None
    return harness.app_server_config(codex_bin=SOURCE_CODEX_BIN)


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

        with Codex(config=_source_config(harness)) as codex:
            thread = codex.thread_start()
            turn = thread.turn(
                [
                    TextInput("  Improve benchmark coverage  "),
                    TextInput("Document the results"),
                ],
                goal=True,
                model="goal-test-model",
                output_schema={
                    "type": "object",
                    "properties": {"summary": {"type": "string"}},
                    "required": ["summary"],
                    "additionalProperties": False,
                },
            )
            result = turn.run()
            requests = harness.responses.wait_for_requests(3)

    usage = result.usage.model_dump(by_alias=True, mode="json") if result.usage else None
    first_body = requests[0].body_json()
    assert {
        "id": result.id,
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
        "initial_input": requests[0].message_input_texts("user")[-2:],
        "model": first_body["model"],
        "output_schema": first_body["text"]["format"]["schema"],
        "continuation_has_objective": (
            "<objective>\nImprove benchmark coverage\n\nDocument the results\n</objective>"
            in _continuation_text(requests[1])
        ),
    } == {
        "id": turn.id,
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
        "initial_input": ["  Improve benchmark coverage  ", "Document the results"],
        "model": "goal-test-model",
        "output_schema": {
            "type": "object",
            "properties": {"summary": {"type": "string"}},
            "required": ["summary"],
            "additionalProperties": False,
        },
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

        with Codex(config=_source_config(harness)) as codex:
            turn = codex.thread_start().turn("Finish the integration suite", goal=True)
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

        with Codex(config=_source_config(harness)) as codex:
            turn = codex.thread_start().turn("Finish in the initial turn", goal=True)
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

        with Codex(config=_source_config(harness)) as codex:
            thread = codex.thread_start()
            previous = codex._client.request(
                "thread/goal/set",
                {
                    "threadId": thread.id,
                    "objective": "Keep the old benchmark objective",
                    "status": ThreadGoalStatus.paused.value,
                    "tokenBudget": 500,
                },
                response_model=ThreadGoalSetResponse,
            ).goal
            result = thread.run("Publish the replacement objective", goal=True)
            persisted = codex._client.request(
                "thread/goal/get",
                {"threadId": thread.id},
                response_model=ThreadGoalGetResponse,
            ).goal
            requests = harness.responses.wait_for_requests(3)

    assert {
        "previous": (previous.objective, previous.status, previous.token_budget),
        "result": (result.status, result.final_response),
        "persisted": (
            persisted.objective if persisted else None,
            persisted.status if persisted else None,
            persisted.token_budget if persisted else None,
        ),
        "continuation_has_replacement": (
            "Publish the replacement objective" in _continuation_text(requests[1])
        ),
        "continuation_has_previous": (
            "Keep the old benchmark objective" in _continuation_text(requests[1])
        ),
    } == {
        "previous": (
            "Keep the old benchmark objective",
            ThreadGoalStatus.paused,
            500,
        ),
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

        with Codex(config=_source_config(harness)) as codex:
            turn = codex.thread_start().turn("Start a goal that needs refinement", goal=True)
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

        with Codex(config=_source_config(harness)) as codex:
            thread = codex.thread_start()
            turn = thread.turn("Start interruptible goal work", goal=True)
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
    """A failed server turn should end the logical operation without another continuation."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("goal-terminal-failure"),
                    ev_failed("goal-terminal-failure", "goal model failed"),
                ]
            )
        )
        harness.responses.enqueue_assistant_message(
            "Recovered with an ordinary turn.",
            response_id="goal-failure-follow-up",
        )

        with Codex(config=_source_config(harness)) as codex:
            thread = codex.thread_start()
            with pytest.raises(RuntimeError, match="goal model failed"):
                thread.run("Fail this goal turn", goal=True)

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
    """Closing a public stream should immediately unregister its logical operation."""
    with AppServerHarness(tmp_path, enable_goals=True) as harness:
        harness.responses.enqueue_sse(
            streaming_response(
                "stream-close",
                "msg-stream-close",
                ["long ", "running ", "goal"],
            ),
            delay_between_events_s=0.5,
        )

        with Codex(config=_source_config(harness)) as codex:
            turn = codex.thread_start().turn("Close this goal stream", goal=True)
            harness.responses.wait_for_requests(1)
            stream = turn.stream()
            stream.close()
            registered_goals = dict(codex._client._router._goal_operations)

    assert registered_goals == {}


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

        with Codex(config=_source_config(harness)) as codex:
            turn = codex.thread_start().turn("Stop the app-server during this goal", goal=True)
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

        with Codex(config=_source_config(harness)) as codex:
            thread = codex.thread_start()
            with pytest.raises(InvalidRequestError) as empty_error:
                thread.turn("   ", goal=True)

            ephemeral = codex.thread_start(ephemeral=True)
            with pytest.raises(InvalidRequestError) as ephemeral_error:
                ephemeral.turn("Persist this goal", goal=True)

            follow_up = thread.run("Run after rejected goals")
            requests = harness.responses.wait_for_requests(1)

    assert {
        "errors": [empty_error.value.message, ephemeral_error.value.message],
        "follow_up": (follow_up.status, follow_up.final_response),
        "request_count": len(requests),
    } == {
        "errors": [
            "goal objective must not be empty",
            f"ephemeral thread does not support goals: {ephemeral.id}",
        ],
        "follow_up": (TurnStatus.completed, "Ordinary turn complete."),
        "request_count": 1,
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

            async with AsyncCodex(config=_source_config(harness)) as codex:
                thread = await codex.thread_start()
                turn = await thread.turn("Finish the async goal", goal=True)
                result = await turn.run()
                requests = harness.responses.wait_for_requests(3)
                with pytest.raises(InvalidRequestError) as steer_error:
                    await turn.steer("Keep working")
                with pytest.raises(InvalidRequestError) as interrupt_error:
                    await turn.interrupt()

        assert {
            "status": result.status,
            "messages": agent_message_texts_from_items(result.items),
            "final_response": result.final_response,
            "continuation_has_objective": (
                "Finish the async goal" in _continuation_text(requests[1])
            ),
            "inactive_errors": [steer_error.value.message, interrupt_error.value.message],
        } == {
            "status": TurnStatus.completed,
            "messages": ["Async initial pass.", "Async goal complete."],
            "final_response": "Async goal complete.",
            "continuation_has_objective": True,
            "inactive_errors": ["no active turn to steer", "no active turn to interrupt"],
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

            async with AsyncCodex(config=_source_config(harness)) as codex:
                thread = await codex.thread_start()
                turn = await thread.turn(
                    "Start an async goal that needs refinement",
                    goal=True,
                )
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

            async with AsyncCodex(config=_source_config(harness)) as codex:
                thread = await codex.thread_start()
                turn = await thread.turn("Start async interruptible goal work", goal=True)
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
