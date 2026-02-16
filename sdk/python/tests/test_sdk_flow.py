from __future__ import annotations

from pathlib import Path

from codex_app_server import AppServerClient, AppServerConfig


HERE = Path(__file__).parent
FAKE = HERE / "fake_app_server.py"


def make_client() -> AppServerClient:
    cfg = AppServerConfig(
        launch_args_override=("python3", str(FAKE)),
        client_name="sdk_test",
    )
    return AppServerClient(cfg)


def test_initialize_thread_turn_flow():
    with make_client() as client:
        init = client.initialize()
        assert init["serverInfo"]["name"] == "fake"

        thread = client.thread_start(model="gpt-5")
        thread_id = thread["thread"]["id"]
        assert thread_id.startswith("thr_")

        turn = client.turn_start(
            thread_id,
            input_items=[{"type": "text", "text": "hello"}],
        )
        turn_id = turn["turn"]["id"]
        done = client.wait_for_turn_completed(turn_id)
        assert done.params["turn"]["status"] == "completed"


def test_list_and_read_and_resume():
    with make_client() as client:
        client.initialize()

        listed = client.thread_list(limit=20)
        assert listed["data"][0]["id"] == "thr_1"

        resumed = client.thread_resume("thr_abc")
        assert resumed["thread"]["id"] == "thr_abc"

        read = client.thread_read("thr_abc", include_turns=True)
        assert read["thread"]["id"] == "thr_abc"


def test_model_list_and_interrupt():
    with make_client() as client:
        client.initialize()
        thread_id = client.thread_start()["thread"]["id"]
        turn_id = client.turn_start(thread_id, input_items=[{"type": "text", "text": "x"}])["turn"]["id"]

        models = client.model_list()
        assert models["data"][0]["id"] == "gpt-5"

        assert client.turn_interrupt(thread_id, turn_id) == {}
