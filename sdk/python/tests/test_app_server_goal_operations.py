from app_server_harness import (
    AppServerHarness,
    ev_assistant_message,
    ev_completed,
    ev_function_call,
    ev_response_created,
    sse,
)
from app_server_helpers import agent_message_texts

from openai_codex import Codex
from openai_codex._goal import _GoalNotificationStream
from openai_codex._run import _collect_turn_result
from openai_codex.generated.notification_registry import notification_turn_id
from openai_codex.generated.v2_all import TurnStatus


def test_private_goal_operation_coalesces_runtime_continuations(tmp_path) -> None:
    """The private engine should expose automatic continuations as one turn."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message(
            "Initial pass complete.",
            response_id="goal-initial",
        )
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("goal-complete-tool"),
                    ev_function_call(
                        "call-goal-complete",
                        "update_goal",
                        '{"status":"complete"}',
                    ),
                    ev_completed("goal-complete-tool"),
                ]
            )
        )
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("goal-final"),
                    ev_assistant_message("msg-goal-final", "Goal complete."),
                    ev_completed("goal-final"),
                ]
            )
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            state, turn_id = codex._client.start_goal_operation(  # noqa: SLF001
                thread.id,
                "Improve benchmark coverage",
            )
            stream = _GoalNotificationStream(
                state,
                state.next_notification,
                lambda: codex._client.unregister_goal_operation(state),  # noqa: SLF001
                lambda: codex._client.cancel_goal_operation(state),  # noqa: SLF001
            )
            events = list(stream)
            result = _collect_turn_result(iter(events), turn_id=turn_id)
            routes = codex._client._router._goal_operations.copy()  # noqa: SLF001
            requests = harness.responses.wait_for_requests(3)

    lifecycle = [event.method for event in events if event.method.startswith("turn/")]
    routed_ids = [
        routed_id
        for event in events
        if (routed_id := notification_turn_id(event.payload)) is not None
    ]
    assert {
        "lifecycle": lifecycle,
        "routed_ids": routed_ids,
        "result": (result.id, result.status, result.final_response),
        "messages": agent_message_texts(events),
        "request_count": len(requests),
        "objective_reached_model": (
            "<objective>\nImprove benchmark coverage\n</objective>"
            in "\n".join(requests[0].message_input_texts("user"))
        ),
        "routes_after_completion": routes,
    } == {
        "lifecycle": ["turn/started", "turn/completed"],
        "routed_ids": [turn_id] * len(routed_ids),
        "result": (turn_id, TurnStatus.completed, "Goal complete."),
        "messages": ["Initial pass complete.", "Goal complete."],
        "request_count": 3,
        "objective_reached_model": True,
        "routes_after_completion": {},
    }
