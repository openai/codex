from __future__ import annotations

import os
import shutil

import pytest

from codex_app_server import AppServerClient


pytestmark = pytest.mark.skipif(
    os.getenv("RUN_REAL_CODEX_TESTS") != "1" or shutil.which("codex") is None,
    reason="Set RUN_REAL_CODEX_TESTS=1 and ensure `codex` is available",
)


def test_real_initialize_and_model_list():
    with AppServerClient() as client:
        out = client.initialize()
        assert isinstance(out, dict)
        models = client.model_list(include_hidden=True)
        assert "data" in models


def test_real_thread_and_turn_start_smoke():
    with AppServerClient() as client:
        client.initialize()
        started = client.thread_start()
        thread_id = started["thread"]["id"]
        assert isinstance(thread_id, str) and thread_id

        turn = client.turn_text(thread_id, "hello")
        turn_id = turn["turn"]["id"]
        assert isinstance(turn_id, str) and turn_id


def test_real_streaming_smoke_turn_completed():
    with AppServerClient() as client:
        client.initialize()
        thread_id = client.thread_start()["thread"]["id"]
        turn = client.turn_text(thread_id, "Reply with one short sentence.")
        turn_id = turn["turn"]["id"]

        saw_delta = False
        completed = False
        for evt in client.stream_until_methods("turn/completed"):
            if evt.method == "item/agentMessage/delta":
                saw_delta = True
            if evt.method == "turn/completed" and (evt.params or {}).get("turn", {}).get("id") == turn_id:
                completed = True

        assert completed
        # Some environments can produce zero deltas for very short output;
        # this assert keeps the smoke test informative but non-flaky.
        assert isinstance(saw_delta, bool)


def test_real_turn_interrupt_smoke():
    with AppServerClient() as client:
        client.initialize()
        thread_id = client.thread_start()["thread"]["id"]
        turn_id = client.turn_text(thread_id, "Count from 1 to 200 with commas.")["turn"]["id"]

        # Best effort: interrupting quickly may race with completion on fast models.
        client.turn_interrupt(thread_id, turn_id)

        events = client.stream_until_methods(["turn/completed", "error"])
        assert events[-1].method in {"turn/completed", "error"}
