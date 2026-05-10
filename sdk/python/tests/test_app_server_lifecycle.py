from __future__ import annotations

from app_server_harness import AppServerHarness
from openai_codex import Codex
from app_server_helpers import request_kind


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
