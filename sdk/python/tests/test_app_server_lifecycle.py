from __future__ import annotations

import asyncio

from app_server_harness import AppServerHarness
from app_server_helpers import request_kind

from openai_codex import AsyncCodex, Codex


def _thread_message_summary(read_response) -> list[tuple[str, str]]:
    """Return persisted user/agent messages from a thread read response."""
    messages: list[tuple[str, str]] = []
    for turn in read_response.thread.turns:
        for item in turn.items:
            root = item.root
            if root.type == "userMessage":
                text = "\n".join(
                    input_item.root.text
                    for input_item in root.content
                    if input_item.root.type == "text"
                )
                messages.append(("user", text))
            if root.type == "agentMessage":
                messages.append(("agent", root.text))
    return messages


def test_thread_set_name_and_read(tmp_path) -> None:
    """Thread naming should round-trip through app-server JSON-RPC."""
    with AppServerHarness(tmp_path) as harness:
        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            thread.set_name("sdk integration thread")
            named = thread.read(include_turns=True)

    assert {"thread_name": named.thread.name} == {
        "thread_name": "sdk integration thread",
    }


def test_sync_and_async_initialization_round_trip_metadata(tmp_path) -> None:
    """Public clients should initialize and start threads through app-server."""

    async def async_scenario(harness: AppServerHarness) -> dict[str, object]:
        async with AsyncCodex(config=harness.app_server_config()) as codex:
            thread = await codex.thread_start()
            server = codex.metadata.serverInfo
            return {
                "thread_id": thread.id,
                "user_agent": codex.metadata.userAgent,
                "server_name": None if server is None else server.name,
                "server_version": None if server is None else server.version,
            }

    with AppServerHarness(tmp_path) as harness:
        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            server = codex.metadata.serverInfo
            sync_summary = {
                "thread_id": thread.id,
                "user_agent": codex.metadata.userAgent,
                "server_name": None if server is None else server.name,
                "server_version": None if server is None else server.version,
            }
        async_summary = asyncio.run(async_scenario(harness))

    assert {
        "sync": {
            "thread_id_present": bool(sync_summary["thread_id"]),
            "user_agent_present": bool(sync_summary["user_agent"]),
            "server_name_present": bool(sync_summary["server_name"]),
            "server_version_present": bool(sync_summary["server_version"]),
        },
        "async": {
            "thread_id_present": bool(async_summary["thread_id"]),
            "user_agent_present": bool(async_summary["user_agent"]),
            "server_name_present": bool(async_summary["server_name"]),
            "server_version_present": bool(async_summary["server_version"]),
        },
    } == {
        "sync": {
            "thread_id_present": True,
            "user_agent_present": True,
            "server_name_present": True,
            "server_version_present": True,
        },
        "async": {
            "thread_id_present": True,
            "user_agent_present": True,
            "server_name_present": True,
            "server_version_present": True,
        },
    }


def test_thread_list_filters_archived_threads(tmp_path) -> None:
    """Thread listing should reflect archive state through app-server."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("active", response_id="list-active")
        harness.responses.enqueue_assistant_message(
            "archived",
            response_id="list-archived",
        )

        with Codex(config=harness.app_server_config()) as codex:
            active_thread = codex.thread_start()
            archived_thread = codex.thread_start()
            active_thread.run("keep this listed")
            archived_thread.run("archive this")
            codex.thread_archive(archived_thread.id)
            active_list = codex.thread_list(archived=False)
            archived_list = codex.thread_list(archived=True)

    expected_ids = {active_thread.id, archived_thread.id}
    assert {
        "active_ids": sorted(thread.id for thread in active_list.data if thread.id in expected_ids),
        "archived_ids": sorted(
            thread.id for thread in archived_list.data if thread.id in expected_ids
        ),
    } == {
        "active_ids": [active_thread.id],
        "archived_ids": [archived_thread.id],
    }


def test_read_include_turns_returns_persisted_history(tmp_path) -> None:
    """Thread.read(include_turns=True) should load real persisted turn items."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("first answer", response_id="read-1")
        harness.responses.enqueue_assistant_message("second answer", response_id="read-2")

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            thread.run("first question")
            thread.run("second question")
            read = thread.read(include_turns=True)

    assert _thread_message_summary(read) == [
        ("user", "first question"),
        ("agent", "first answer"),
        ("user", "second question"),
        ("agent", "second answer"),
    ]


def test_async_lifecycle_methods_round_trip(tmp_path) -> None:
    """Async lifecycle helpers should preserve the same app-server thread state."""

    async def scenario() -> None:
        """Exercise async wrappers over one materialized thread."""
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_assistant_message(
                "async materialized",
                response_id="async-lifecycle",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start()
                run_result = await thread.run("materialize async thread")
                await thread.set_name("async lifecycle")
                named = await thread.read()
                resumed = await codex.thread_resume(thread.id)
                forked = await codex.thread_fork(thread.id)
                archive_response = await codex.thread_archive(thread.id)
                unarchived = await codex.thread_unarchive(thread.id)

        assert {
            "run_final_response": run_result.final_response,
            "named_thread": named.thread.name,
            "resumed_id": resumed.id,
            "forked_is_distinct": forked.id != thread.id,
            "archive_response": archive_response.model_dump(by_alias=True, mode="json"),
            "unarchived_id": unarchived.id,
        } == {
            "run_final_response": "async materialized",
            "named_thread": "async lifecycle",
            "resumed_id": thread.id,
            "forked_is_distinct": True,
            "archive_response": {},
            "unarchived_id": thread.id,
        }

    asyncio.run(scenario())


def test_thread_fork_returns_distinct_thread(tmp_path) -> None:
    """Thread fork should return a distinct thread for a persisted rollout."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("materialized", response_id="fork-seed")

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            seeded = thread.run("materialize this thread before fork")
            forked = codex.thread_fork(thread.id)

    assert {
        "seeded_response": seeded.final_response,
        "forked_is_distinct": forked.id != thread.id,
    } == {
        "seeded_response": "materialized",
        "forked_is_distinct": True,
    }


def test_archive_unarchive_round_trip_uses_materialized_rollout(tmp_path) -> None:
    """Archive helpers should work once the app-server has persisted a rollout."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("materialized", response_id="archive-seed")

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            seeded = thread.run("materialize this thread before archive")
            archived = codex.thread_archive(thread.id)
            unarchived = codex.thread_unarchive(thread.id)
            read = unarchived.read()

    assert {
        "seeded_response": seeded.final_response,
        "archive_response": archived.model_dump(by_alias=True, mode="json"),
        "unarchived_id": unarchived.id,
        "read_id": read.thread.id,
    } == {
        "seeded_response": "materialized",
        "archive_response": {},
        "unarchived_id": thread.id,
        "read_id": thread.id,
    }


def test_models_rpc(tmp_path) -> None:
    """Model listing should go through the pinned app-server method."""
    with AppServerHarness(tmp_path) as harness:
        with Codex(config=harness.app_server_config()) as codex:
            models = codex.models(include_hidden=True)

    assert {
        "models_payload_has_data": isinstance(
            models.model_dump(by_alias=True, mode="json").get("data"),
            list,
        ),
    } == {"models_payload_has_data": True}


def test_compact_rpc_hits_mock_responses(tmp_path) -> None:
    """Compaction should run through app-server and hit the mock Responses boundary."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("history", response_id="compact-history")
        harness.responses.enqueue_assistant_message(
            "compact summary",
            response_id="compact-summary",
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            run_result = thread.run("create history")
            compact_response = thread.compact()
            requests = harness.responses.wait_for_requests(2)

    assert {
        "run_final_response": run_result.final_response,
        "compact_response": compact_response.model_dump(
            by_alias=True,
            mode="json",
        ),
        "request_kinds": [request_kind(request.path) for request in requests],
    } == {
        "run_final_response": "history",
        "compact_response": {},
        "request_kinds": ["responses", "responses"],
    }
