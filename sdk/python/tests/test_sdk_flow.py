from __future__ import annotations

from pathlib import Path

import asyncio

from codex_app_server import AppServerClient, AppServerConfig, AsyncAppServerClient


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

        turn = client.turn_start(thread_id, "hello")
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


def test_notebook_helper_run_text_turn():
    with make_client() as client:
        client.initialize()
        thread_id = client.thread_start()["thread"]["id"]
        text, completed = client.run_text_turn(thread_id, "hello")
        assert text == "hello world"
        assert completed.params["turn"]["status"] == "completed"


def test_turn_start_accepts_single_item_dict_and_turn_text_alias():
    with make_client() as client:
        client.initialize()
        thread_id = client.thread_start()["thread"]["id"]

        turn = client.turn_start(thread_id, {"type": "text", "text": "hello"})
        done = client.wait_for_turn_completed(turn["turn"]["id"])
        assert done.params["turn"]["status"] == "completed"

        turn2 = client.turn_text(thread_id, "hello")
        done2 = client.wait_for_turn_completed(turn2["turn"]["id"])
        assert done2.params["turn"]["status"] == "completed"


def test_stream_until_methods_accepts_single_method_string():
    with make_client() as client:
        client.initialize()
        thread_id = client.thread_start()["thread"]["id"]
        client.turn_text(thread_id, "hello")

        events = client.stream_until_methods("turn/completed")
        assert events[-1].method == "turn/completed"


def test_async_client_wrapper():
    async def _run():
        cfg = AppServerConfig(launch_args_override=("python3", str(FAKE)))
        async with AsyncAppServerClient(cfg) as client:
            await client.initialize()
            thread = await client.thread_start(model="gpt-5")
            thread_id = thread["thread"]["id"]

            listed = await client.thread_list(limit=5)
            assert listed["data"][0]["id"] == "thr_1"

            turn = await client.turn_text(thread_id, "hello")
            done = await client.wait_for_turn_completed(turn["turn"]["id"])
            assert done.params["turn"]["status"] == "completed"

            _thread_id, answer = await client.ask("hello", thread_id=thread_id)
            assert _thread_id == thread_id
            assert answer == "hello world"

    asyncio.run(_run())


def test_typed_wrappers_and_ask_helper_and_approval_flow():
    with make_client() as client:
        client.initialize()

        started = client.thread_start_typed(model="gpt-5")
        assert started.thread.id.startswith("thr_")

        # Ensure default approval handler responds to server requests.
        turn = client.turn_start(
            started.thread.id,
            input_items=[{"type": "text", "text": "hello"}],
            requireApproval=True,
        )
        done = client.wait_for_turn_completed(turn["turn"]["id"])
        assert done.params["turn"]["status"] == "completed"

        thread_id, answer = client.ask("hello again", thread_id=started.thread.id)
        assert thread_id == started.thread.id
        assert answer == "hello world"
