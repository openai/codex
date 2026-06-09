import asyncio
import time

import pytest
from app_server_harness import (
    AppServerHarness,
    SseGate,
    ev_completed_with_usage,
    ev_function_call,
    ev_response_created,
    sse,
)
from app_server_helpers import agent_message_texts_from_items, streaming_response

from openai_codex import AsyncCodex, Codex
from openai_codex.generated.v2_all import (
    ThreadGoalGetResponse,
    ThreadGoalStatus,
    TurnStatus,
)


def _enqueue_steerable_goal(
    harness: AppServerHarness,
    prefix: str,
    *,
    initial_text: str,
    continuation_chunks: list[str],
    final_text: str,
) -> SseGate:
    continuation_gate = SseGate()
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
        gate=continuation_gate,
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
    return continuation_gate


def _enqueue_interruptible_goal(
    harness: AppServerHarness,
    prefix: str,
    *,
    initial_text: str,
    continuation_chunks: list[str],
    follow_up_text: str,
) -> SseGate:
    continuation_gate = SseGate()
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
        gate=continuation_gate,
    )
    harness.responses.enqueue_assistant_message(
        follow_up_text,
        response_id=f"{prefix}-follow-up",
    )
    return continuation_gate


def _stale_routed_turn_id(turn) -> None:
    goal = turn._goal
    assert goal is not None
    goal_state = goal.state
    deadline = time.monotonic() + 5
    with goal_state._condition:
        while goal_state.current_turn_id in {None, turn.id}:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise AssertionError("continuation turn was not routed")
            goal_state._condition.wait(remaining)
        goal_state.current_turn_id = turn.id


def test_goal_steer_targets_an_active_continuation(tmp_path) -> None:
    """Steering should reach the active server turn while returning the logical ID."""
    with AppServerHarness(tmp_path) as harness:
        continuation_gate = _enqueue_steerable_goal(
            harness,
            "steer",
            initial_text="Initial work complete.",
            continuation_chunks=["continuation ", "work"],
            final_text="Steered goal complete.",
        )

        with Codex(config=harness.app_server_config()) as codex:
            try:
                turn = codex.thread_start().start_goal("Start a goal that needs refinement")
                continuation_gate.wait_until_ready()
                steer = turn.steer("Prioritize the edge-case coverage.")
            finally:
                continuation_gate.release.set()
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


def test_goal_steer_retries_the_server_reported_rollover_turn(tmp_path) -> None:
    """Steering should recover when routed state trails the server continuation."""
    with AppServerHarness(tmp_path) as harness:
        continuation_gate = _enqueue_steerable_goal(
            harness,
            "steer-rollover",
            initial_text="Initial rollover work complete.",
            continuation_chunks=["rollover ", "work"],
            final_text="Rollover steering complete.",
        )

        with Codex(config=harness.app_server_config()) as codex:
            try:
                turn = codex.thread_start().start_goal("Steer while routing catches up")
                continuation_gate.wait_until_ready()
                _stale_routed_turn_id(turn)
                steer = turn.steer("Steer through the rollover window.")
            finally:
                continuation_gate.release.set()
            result = turn.run()
            requests = harness.responses.wait_for_requests(4)

    assert {
        "steer_id": steer.turn_id,
        "result": (result.id, result.status, result.final_response),
        "steering_input": requests[2].message_input_texts("user")[-1:],
    } == {
        "steer_id": turn.id,
        "result": (turn.id, TurnStatus.completed, "Rollover steering complete."),
        "steering_input": ["Steer through the rollover window."],
    }


@pytest.mark.parametrize("stale_route", [False, True])
def test_goal_interrupts_active_and_stale_routed_continuations(tmp_path, stale_route) -> None:
    """Interruption should stop active work despite notification rollover lag."""
    with AppServerHarness(tmp_path) as harness:
        continuation_gate = _enqueue_interruptible_goal(
            harness,
            "interrupt",
            initial_text="Initial work complete.",
            continuation_chunks=["still ", "working"],
            follow_up_text="Ordinary follow-up complete.",
        )

        with Codex(config=harness.app_server_config()) as codex:
            try:
                thread = codex.thread_start()
                turn = thread.start_goal("Start interruptible goal work")
                continuation_gate.wait_until_ready()
                if stale_route:
                    _stale_routed_turn_id(turn)
                interrupt = turn.interrupt()
            finally:
                continuation_gate.release.set()
            interrupted = turn.run()
            follow_up = thread.run("Continue with an ordinary turn")
            requests = harness.responses.wait_for_requests(3)
            persisted = codex._client.request(
                "thread/goal/get",
                {"threadId": thread.id},
                response_model=ThreadGoalGetResponse,
            ).goal

    assert {
        "interrupt": interrupt.model_dump(by_alias=True, mode="json"),
        "goal_status": persisted.status if persisted else None,
        "interrupted": (interrupted.id, interrupted.status),
        "follow_up": (follow_up.status, follow_up.final_response),
        "request_count": len(requests),
    } == {
        "interrupt": {},
        "goal_status": ThreadGoalStatus.paused,
        "interrupted": (turn.id, TurnStatus.interrupted),
        "follow_up": (TurnStatus.completed, "Ordinary follow-up complete."),
        "request_count": 3,
    }


def test_async_goal_steer_retries_the_server_reported_rollover_turn(tmp_path) -> None:
    """Async steering should recover when routed state trails the continuation."""

    async def scenario() -> None:
        with AppServerHarness(tmp_path) as harness:
            continuation_gate = _enqueue_steerable_goal(
                harness,
                "async-steer",
                initial_text="Async initial work complete.",
                continuation_chunks=["async continuation ", "work"],
                final_text="Async steered goal complete.",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                try:
                    thread = await codex.thread_start()
                    turn = await thread.start_goal("Start an async goal that needs refinement")
                    await asyncio.to_thread(continuation_gate.wait_until_ready)
                    await asyncio.to_thread(_stale_routed_turn_id, turn)
                    steer = await turn.steer("Prioritize async edge-case coverage.")
                finally:
                    continuation_gate.release.set()
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


def test_async_goal_interrupt_retries_the_server_reported_rollover_turn(tmp_path) -> None:
    """Async interruption should survive notification lag during rollover."""

    async def scenario() -> None:
        with AppServerHarness(tmp_path) as harness:
            continuation_gate = _enqueue_interruptible_goal(
                harness,
                "async-interrupt",
                initial_text="Async initial work complete.",
                continuation_chunks=["async still ", "working"],
                follow_up_text="Async ordinary follow-up complete.",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                try:
                    thread = await codex.thread_start()
                    turn = await thread.start_goal("Start async interruptible goal work")
                    await asyncio.to_thread(continuation_gate.wait_until_ready)
                    await asyncio.to_thread(_stale_routed_turn_id, turn)
                    interrupt = await turn.interrupt()
                finally:
                    continuation_gate.release.set()
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
