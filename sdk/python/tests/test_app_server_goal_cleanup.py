import _thread
import asyncio
import threading
import time

import pytest
from app_server_harness import AppServerHarness, SseGate, ev_failed, ev_response_created, sse
from app_server_helpers import streaming_response

from openai_codex import AsyncCodex, Codex
from openai_codex.api import _MAX_THREAD_GOAL_OBJECTIVE_CHARS
from openai_codex.errors import InvalidRequestError, TransportClosedError
from openai_codex.generated.v2_all import (
    IdleThreadStatus,
    ThreadGoalGetResponse,
    ThreadGoalStatus,
    TurnStatus,
)


def _queued_notification(state):
    notifications = state._notifications
    with notifications.not_empty:
        if not notifications.not_empty.wait_for(lambda: bool(notifications.queue), timeout=5):
            raise AssertionError("goal notification was not queued")
        return notifications.queue[0]


def _wait_until_consumed(state, marker) -> None:
    notifications = state._notifications
    with notifications.not_full:
        if not notifications.not_full.wait_for(
            lambda: all(item is not marker for item in notifications.queue),
            timeout=5,
        ):
            raise AssertionError("goal stream did not begin collecting")


def test_terminal_goal_failure_preserves_status_and_releases_routing(tmp_path) -> None:
    """A failed server turn should stop rollover work and leave the thread usable."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("goal-terminal-failure"),
                    ev_failed(
                        "goal-terminal-failure",
                        "goal model failed",
                        code="insufficient_quota",
                    ),
                ]
            )
        )
        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            with pytest.raises(RuntimeError) as error:
                thread.run_goal("Fail this goal turn")

            persisted = codex._client.request(
                "thread/goal/get",
                {"threadId": thread.id},
                response_model=ThreadGoalGetResponse,
            ).goal
            harness.responses.enqueue_assistant_message(
                "Recovered with an ordinary turn.",
                response_id="goal-failure-follow-up",
            )
            follow_up = thread.run("Run after the goal failure")
            harness.responses.wait_for_requests(2)
            requests = harness.responses.requests()

    assert {
        "error": str(error.value),
        "goal_status": persisted.status if persisted else None,
        "follow_up": (follow_up.status, follow_up.final_response),
        "request_count": len(requests),
        "follow_up_input": requests[1].message_input_texts("user")[-1:],
    } == {
        "error": "Quota exceeded. Check your plan and billing details.",
        "goal_status": ThreadGoalStatus.usage_limited,
        "follow_up": (TurnStatus.completed, "Recovered with an ordinary turn."),
        "request_count": 2,
        "follow_up_input": ["Run after the goal failure"],
    }


def test_sync_goal_run_cancellation_stops_work_and_releases_routing(tmp_path) -> None:
    """Keyboard interruption should pause and interrupt a running logical goal."""
    with AppServerHarness(tmp_path) as harness:
        work_gate = SseGate()
        harness.responses.enqueue_sse(
            streaming_response(
                "cancelled-sync-run",
                "msg-cancelled-sync-run",
                ["cancelled ", "sync ", "goal"],
            ),
            delay_between_events_s=0.5,
            gate=work_gate,
        )
        harness.responses.enqueue_assistant_message(
            "Sync follow-up complete.",
            response_id="cancelled-sync-run-follow-up",
        )

        with Codex(config=harness.app_server_config()) as codex:
            try:
                thread = codex.thread_start()
                turn = thread.start_goal("Cancel this running sync goal")
                goal = turn._goal
                assert goal is not None
                goal_state = goal.state
                work_gate.wait_until_ready()
                marker = _queued_notification(goal_state)

                worker_errors: list[BaseException] = []

                def interrupt_when_collecting() -> None:
                    try:
                        _wait_until_consumed(goal_state, marker)
                    except BaseException as exc:
                        worker_errors.append(exc)
                    finally:
                        _thread.interrupt_main()
                        work_gate.release.set()

                interrupter = threading.Thread(target=interrupt_when_collecting)
                try:
                    with pytest.raises(KeyboardInterrupt):
                        interrupter.start()
                        turn.run()
                finally:
                    interrupter.join()
            finally:
                work_gate.release.set()

            deadline = time.monotonic() + 5
            while True:
                current = thread.read()
                if isinstance(current.thread.status.root, IdleThreadStatus):
                    break
                if time.monotonic() >= deadline:
                    raise AssertionError("cancelled sync goal turn did not stop")
                time.sleep(0.01)

            persisted = codex._client.request(
                "thread/goal/get",
                {"threadId": thread.id},
                response_model=ThreadGoalGetResponse,
            ).goal
            registered_goals = dict(codex._client._router._goal_operations)
            follow_up = thread.run("Continue after cancelling the sync goal")
            requests = harness.responses.wait_for_requests(2)

    assert {
        "goal_status": persisted.status if persisted else None,
        "follow_up": (follow_up.status, follow_up.final_response),
        "request_count": len(requests),
        "registered_goals": registered_goals,
        "interrupter_errors": [str(error) for error in worker_errors],
    } == {
        "goal_status": ThreadGoalStatus.paused,
        "follow_up": (TurnStatus.completed, "Sync follow-up complete."),
        "request_count": 2,
        "registered_goals": {},
        "interrupter_errors": [],
    }


def test_closing_goal_stream_releases_real_process_routing(tmp_path) -> None:
    """Closing a stream should release SDK routing without pausing the persisted goal."""
    with AppServerHarness(tmp_path) as harness:
        work_gate = SseGate()
        harness.responses.enqueue_sse(
            streaming_response(
                "stream-close",
                "msg-stream-close",
                ["long ", "running ", "goal"],
            ),
            delay_between_events_s=0.5,
            gate=work_gate,
        )

        with Codex(config=harness.app_server_config()) as codex:
            try:
                thread = codex.thread_start()
                turn = thread.start_goal("Close this goal stream")
                work_gate.wait_until_ready()
                stream = turn.stream()
                stream.close()
                registered_goals = dict(codex._client._router._goal_operations)
                persisted = codex._client.request(
                    "thread/goal/get",
                    {"threadId": thread.id},
                    response_model=ThreadGoalGetResponse,
                ).goal
            finally:
                work_gate.release.set()

    assert {
        "registered_goals": registered_goals,
        "persisted_status": persisted.status if persisted else None,
    } == {
        "registered_goals": {},
        "persisted_status": ThreadGoalStatus.active,
    }


def test_app_server_exit_unblocks_goal_stream_and_releases_routing(tmp_path) -> None:
    """A real transport death should wake the logical reader and clean up routing."""
    with AppServerHarness(tmp_path) as harness:
        work_gate = SseGate()
        harness.responses.enqueue_sse(
            streaming_response(
                "transport-exit",
                "msg-transport-exit",
                ["long ", "running ", "goal"],
            ),
            delay_between_events_s=0.5,
            gate=work_gate,
        )

        with Codex(config=harness.app_server_config()) as codex:
            try:
                turn = codex.thread_start().start_goal("Stop the app-server during this goal")
                work_gate.wait_until_ready()
                process = codex._client._proc
                assert process is not None
                process.terminate()
                process.wait(timeout=5)
            finally:
                work_gate.release.set()

            with pytest.raises(TransportClosedError):
                turn.run()
            registered_goals = dict(codex._client._router._goal_operations)

    assert registered_goals == {}


def test_failed_goal_starts_release_routing_without_model_requests(tmp_path) -> None:
    """Validation failures should leave the client and persisted thread usable."""
    with AppServerHarness(tmp_path) as harness:
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
                thread.start_goal("x" * (_MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1))

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
    with AppServerHarness(tmp_path, enable_goals=False) as harness:
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
    with AppServerHarness(tmp_path) as harness:
        work_gate = SseGate()
        harness.responses.enqueue_sse(
            streaming_response(
                "active-ordinary-turn",
                "msg-active-ordinary-turn",
                ["ordinary ", "work"],
            ),
            delay_between_events_s=0.5,
            gate=work_gate,
        )

        with Codex(config=harness.app_server_config()) as codex:
            try:
                thread = codex.thread_start()
                ordinary = thread.turn("Keep this ordinary turn active")
                work_gate.wait_until_ready()
                with pytest.raises(InvalidRequestError) as error:
                    thread.start_goal("Do not replace active work")
                ordinary.interrupt()
            finally:
                work_gate.release.set()
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


def test_async_goal_run_cancellation_stops_work_and_releases_routing(tmp_path) -> None:
    """Task cancellation should pause and interrupt a running logical goal."""

    async def scenario() -> None:
        with AppServerHarness(tmp_path) as harness:
            work_gate = SseGate()
            harness.responses.enqueue_sse(
                streaming_response(
                    "cancelled-async-run",
                    "msg-cancelled-async-run",
                    ["cancelled ", "async ", "goal"],
                ),
                delay_between_events_s=0.5,
                gate=work_gate,
            )
            harness.responses.enqueue_assistant_message(
                "Async run follow-up complete.",
                response_id="cancelled-async-run-follow-up",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                try:
                    thread = await codex.thread_start()
                    turn = await thread.start_goal("Cancel this running async goal")
                    goal = turn._goal
                    assert goal is not None
                    goal_state = goal.state
                    marker = _queued_notification(goal_state)
                    running = asyncio.create_task(turn.run())
                    await asyncio.to_thread(work_gate.wait_until_ready)
                    await asyncio.to_thread(_wait_until_consumed, goal_state, marker)
                    running.cancel()
                    with pytest.raises(asyncio.CancelledError):
                        await running
                finally:
                    work_gate.release.set()

                deadline = time.monotonic() + 5
                while True:
                    current = await thread.read()
                    if isinstance(current.thread.status.root, IdleThreadStatus):
                        break
                    if time.monotonic() >= deadline:
                        raise AssertionError("cancelled async goal turn did not stop")
                    await asyncio.sleep(0.01)

                persisted = await codex._client.request(
                    "thread/goal/get",
                    {"threadId": thread.id},
                    response_model=ThreadGoalGetResponse,
                )
                registered_goals = dict(codex._client._sync._router._goal_operations)
                follow_up = await thread.run("Continue after cancelling the async goal")
                requests = await asyncio.to_thread(harness.responses.wait_for_requests, 2)

        assert {
            "goal_status": persisted.goal.status if persisted.goal else None,
            "follow_up": (follow_up.status, follow_up.final_response),
            "request_count": len(requests),
            "registered_goals": registered_goals,
        } == {
            "goal_status": ThreadGoalStatus.paused,
            "follow_up": (TurnStatus.completed, "Async run follow-up complete."),
            "request_count": 2,
            "registered_goals": {},
        }

    asyncio.run(scenario())


def test_async_goal_start_cancellation_interrupts_work_and_releases_routing(tmp_path) -> None:
    """Cancelling startup should pause and interrupt the physical goal turn."""

    async def scenario() -> None:
        with AppServerHarness(tmp_path) as harness:
            work_gate = SseGate()
            harness.responses.enqueue_sse(
                streaming_response(
                    "cancelled-start",
                    "msg-cancelled-start",
                    ["cancelled ", "goal ", "work"],
                ),
                delay_between_events_s=0.5,
                gate=work_gate,
            )
            harness.responses.enqueue_assistant_message(
                "Async follow-up complete.",
                response_id="cancelled-start-follow-up",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                try:
                    thread = await codex.thread_start()
                    startup = asyncio.create_task(
                        thread.start_goal("Cancel this goal during startup")
                    )
                    await asyncio.sleep(0)
                    work_gate.wait_until_ready()
                    startup.cancel()
                    with pytest.raises(asyncio.CancelledError):
                        await asyncio.wait_for(startup, timeout=1)
                finally:
                    work_gate.release.set()

                router = codex._client._sync._router
                deadline = time.monotonic() + 5
                while True:
                    current = await thread.read()
                    with router._lock:
                        registered_goals = dict(router._goal_operations)
                    if (
                        isinstance(current.thread.status.root, IdleThreadStatus)
                        and not registered_goals
                    ):
                        break
                    if time.monotonic() >= deadline:
                        raise AssertionError("cancelled goal startup did not stop")
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
            "thread_status": current.thread.status.root,
        } == {
            "goal_status": ThreadGoalStatus.paused,
            "follow_up": (TurnStatus.completed, "Async follow-up complete."),
            "request_count": 2,
            "registered_goals": {},
            "thread_status": IdleThreadStatus(type="idle"),
        }

    asyncio.run(scenario())
