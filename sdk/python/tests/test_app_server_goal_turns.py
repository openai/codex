import asyncio

import pytest
from app_server_harness import (
    AppServerHarness,
    ev_assistant_message,
    ev_completed_with_usage,
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
from openai_codex.api import _MAX_THREAD_GOAL_OBJECTIVE_CHARS
from openai_codex.errors import InvalidRequestError
from openai_codex.generated.notification_registry import notification_turn_id
from openai_codex.generated.v2_all import (
    AgentMessageDeltaNotification,
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


def _continuation_text(request) -> str:
    return "\n".join(request.message_input_texts("user"))


def test_sync_goal_run_aggregates_automatic_continuation(tmp_path) -> None:
    """The public result should cover the initial and automatic continuation turns."""
    with AppServerHarness(tmp_path) as harness:
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
    with AppServerHarness(tmp_path) as harness:
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
        "routed_ids": sorted(set(routed_ids)),
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
        "routed_ids": [turn.id],
        "deltas": ["initial ", "pass", "goal ", "complete"],
        "messages": ["initial pass", "goal complete"],
        "completion_statuses": [TurnStatus.completed],
    }


def test_goal_can_complete_within_the_initial_server_turn(tmp_path) -> None:
    """Completing the goal before turn end should not create a continuation turn."""
    with AppServerHarness(tmp_path) as harness:
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
    with AppServerHarness(tmp_path) as harness:
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


def test_goal_length_validation_preserves_an_existing_goal(tmp_path) -> None:
    """An invalid replacement should fail before clearing the stored goal."""
    with AppServerHarness(tmp_path) as harness:
        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            previous = codex._client.thread_goal_set(
                thread.id,
                objective="Keep the existing objective",
                status=ThreadGoalStatus.paused,
            ).goal
            with pytest.raises(ValueError) as error:
                thread.start_goal("x" * (_MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1))
            persisted = codex._client.request(
                "thread/goal/get",
                {"threadId": thread.id},
                response_model=ThreadGoalGetResponse,
            ).goal
            requests = harness.responses.requests()

    assert {
        "error": str(error.value),
        "persisted": persisted,
        "model_requests": requests,
    } == {
        "error": "goal objective must be at most 4000 characters",
        "persisted": previous,
        "model_requests": [],
    }


def test_async_goal_run_matches_sync_logical_result(tmp_path) -> None:
    """The async public API should aggregate the same real continuation lifecycle."""

    async def scenario() -> None:
        with AppServerHarness(tmp_path) as harness:
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
