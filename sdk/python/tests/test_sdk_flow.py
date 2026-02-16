from __future__ import annotations

from pathlib import Path

import asyncio

import pytest

from codex_app_server import AppServerClient, AppServerConfig, AsyncAppServerClient
from codex_app_server.errors import MethodNotFoundError, RetryLimitExceededError, ServerBusyError


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




def test_ask_result_and_stream_text_helpers():
    with make_client() as client:
        client.initialize()

        started = client.thread_start()
        thread_id = started["thread"]["id"]

        result = client.ask_result("hello", thread_id=thread_id)
        assert result.thread_id == thread_id
        assert result.text == "hello world"
        assert result.completed.method == "turn/completed"

        chunks = list(client.stream_text(thread_id, "hello"))
        assert chunks == ["hello ", "world"]


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

        text_turn = client.turn_text_typed(started.thread.id, "hello")
        assert text_turn.turn.id.startswith("turn_")
        client.wait_for_turn_completed(text_turn.turn.id)

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


def test_conversation_and_schema_wrappers():
    with make_client() as client:
        client.initialize()

        conv = client.conversation_start(model="gpt-5")
        assert conv.thread_id.startswith("thr_")

        turn = conv.turn_text_schema("hello")
        assert turn.turn.id.startswith("turn_")
        client.wait_for_turn_completed(turn.turn.id)

        answer = conv.ask("hello again")
        assert answer == "hello world"

        streamed = [evt.method for evt in conv.stream("stream me")]
        assert "item/agentMessage/delta" in streamed
        assert streamed[-1] == "turn/completed"

        started = client.thread_start_schema(model="gpt-5")
        assert started.thread.id.startswith("thr_")

        listed = client.thread_list_schema(limit=1)
        assert listed.data[0].id == "thr_1"


def test_async_conversation_and_schema_wrappers():
    async def _run():
        cfg = AppServerConfig(launch_args_override=("python3", str(FAKE)))
        async with AsyncAppServerClient(cfg) as client:
            await client.initialize()

            started_typed = await client.thread_start_typed(model="gpt-5")
            assert started_typed.thread.id.startswith("thr_")

            conv = await client.conversation_start(model="gpt-5")
            assert conv.thread_id.startswith("thr_")

            answer = await conv.ask("hello")
            assert answer == "hello world"

            ask_result = await conv.ask_result("hello")
            assert ask_result.text == "hello world"
            assert ask_result.thread_id == conv.thread_id

            streamed = []
            async for evt in conv.stream("hello"):
                streamed.append(evt.method)
            assert streamed[-1] == "turn/completed"

            text_chunks = [chunk async for chunk in conv.stream_text("hello")]
            assert text_chunks == ["hello ", "world"]

            turn = await client.turn_text_schema(conv.thread_id, "hello")
            assert turn.turn.id.startswith("turn_")

            started = await client.thread_start_schema(model="gpt-5")
            assert started.thread.id.startswith("thr_")

            resumed = await client.thread_resume_schema(conv.thread_id)
            assert resumed.thread.id == conv.thread_id

            typed_turn = await client.turn_text_typed(conv.thread_id, "typed hello")
            assert typed_turn.turn.id.startswith("turn_")

            first = await client.next_notification()
            parsed = await client.parse_notification_typed(first)
            assert parsed is not None

    asyncio.run(_run())


def test_extended_typed_and_schema_wrappers_plus_notification_parsing():
    with make_client() as client:
        client.initialize()
        started = client.thread_start_typed()

        resumed = client.thread_resume_typed(started.thread.id)
        assert resumed.thread.id == started.thread.id

        read = client.thread_read_typed(started.thread.id, include_turns=True)
        assert read.thread.id == started.thread.id

        listed = client.thread_list_typed(limit=1)
        assert listed.data[0].id == "thr_1"

        models = client.model_list_typed()
        assert models.data[0].id == "gpt-5"

        client.turn_text(started.thread.id, "hello")
        first = client.next_notification()
        typed = client.parse_notification_typed(first)
        assert typed is not None

        second = client.next_notification()
        schema = client.parse_notification_schema(second)
        assert schema is not None


def test_jsonrpc_error_mapping_and_retry_helper():
    with make_client() as client:
        client.initialize()

        with pytest.raises(MethodNotFoundError):
            client.request("missing/method")

        out = client.request_with_retry_on_overload("test/overload-once", max_attempts=2)
        assert out["ok"] is True

        with pytest.raises(RetryLimitExceededError):
            client.request("test/always-overload")

        with pytest.raises(ServerBusyError):
            client.request_with_retry_on_overload("test/always-overload", max_attempts=2)


def test_thread_lifecycle_and_steer_typed_schema_helpers():
    with make_client() as client:
        client.initialize()
        started = client.thread_start_typed(model="gpt-5")

        forked = client.thread_fork_typed(started.thread.id)
        assert forked.thread.id.startswith("thr_")

        _ = client.thread_archive_typed(started.thread.id)
        unarchived = client.thread_unarchive_schema(started.thread.id)
        assert unarchived.thread.id == started.thread.id

        _ = client.thread_set_name_typed(started.thread.id, "renamed")
        while True:
            evt = client.next_notification()
            if evt.method == "thread/nameUpdated":
                parsed = client.parse_notification_typed(evt)
                assert parsed is not None
                assert getattr(parsed, "thread_name", None) == "renamed"
                break

        turn = client.turn_text_typed(started.thread.id, "hello")
        steered = client.turn_steer_typed(started.thread.id, turn.turn.id, "continue")
        assert steered.turn_id == turn.turn.id


def test_extended_notification_parsers_include_item_and_usage_events():
    with make_client() as client:
        client.initialize()
        thread_id = client.thread_start()["thread"]["id"]
        turn_id = client.turn_text(thread_id, "hello")["turn"]["id"]

        saw_item_started = False
        saw_item_completed = False
        saw_usage = False
        while True:
            n = client.next_notification()
            typed = client.parse_notification_typed(n)
            schema = client.parse_notification_schema(n)
            if n.method == "item/started":
                saw_item_started = typed is not None and schema is not None
            if n.method == "item/completed":
                saw_item_completed = typed is not None and schema is not None
            if n.method == "thread/tokenUsageUpdated":
                saw_usage = typed is not None and schema is not None
            if n.method == "turn/completed" and (n.params or {}).get("turn", {}).get("id") == turn_id:
                # consume through usage event emitted after completion in fake server
                continue
            if saw_item_started and saw_item_completed and saw_usage:
                break
