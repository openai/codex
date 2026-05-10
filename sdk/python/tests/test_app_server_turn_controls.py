from __future__ import annotations

from app_server_harness import AppServerHarness
from openai_codex import Codex, TextInput
from openai_codex.generated.v2_all import TurnStatus
from app_server_helpers import agent_message_texts, streaming_response


def test_turn_steer_adds_follow_up_input(tmp_path) -> None:
    """Steering an active turn should create a follow-up Responses request."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            streaming_response("steer-first", "msg-steer-first", ["before steer"]),
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
        "agent_messages": agent_message_texts(events),
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
            streaming_response(
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
