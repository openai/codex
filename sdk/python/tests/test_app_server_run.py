from __future__ import annotations

import asyncio

import pytest
from app_server_harness import (
    AppServerHarness,
    ev_assistant_message,
    ev_completed,
    ev_completed_with_usage,
    ev_failed,
    ev_response_created,
    sse,
)
from app_server_helpers import (
    agent_message_texts_from_items,
    assistant_message_with_phase,
)

from openai_codex import AsyncCodex, Codex
from openai_codex.generated.v2_all import MessagePhase


def test_sync_thread_run_uses_mock_responses(
    tmp_path,
) -> None:
    """Drive Thread.run through the pinned app-server and inspect the HTTP request."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("Hello from the mock.", response_id="run-1")

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            result = thread.run("hello")

        request = harness.responses.single_request()

    body = request.body_json()
    assert {
        "final_response": result.final_response,
        "agent_messages": agent_message_texts_from_items(result.items),
        "has_usage": result.usage is not None,
        "request_model": body["model"],
        "request_stream": body["stream"],
        "request_user_texts": request.message_input_texts("user")[-1:],
    } == {
        "final_response": "Hello from the mock.",
        "agent_messages": ["Hello from the mock."],
        "has_usage": True,
        "request_model": "mock-model",
        "request_stream": True,
        "request_user_texts": ["hello"],
    }


def test_run_params_and_usage_cross_app_server_boundary(tmp_path) -> None:
    """Thread.run should pass overrides and collect app-server token usage."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("run-overrides"),
                    ev_assistant_message("msg-run-overrides", "overrides applied"),
                    ev_completed_with_usage(
                        "run-overrides",
                        input_tokens=11,
                        cached_input_tokens=3,
                        output_tokens=7,
                        reasoning_output_tokens=5,
                        total_tokens=18,
                    ),
                ]
            )
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            result = thread.run(
                "use overrides",
                model="mock-model-override",
            )
            request = harness.responses.single_request()

    usage_payload = None
    if result.usage is not None:
        dumped_usage = result.usage.model_dump(by_alias=True, mode="json")
        usage_payload = {
            "last": dumped_usage["last"],
            "total": dumped_usage["total"],
        }
    assert {
        "final_response": result.final_response,
        "request_model": request.body_json()["model"],
        "usage": usage_payload,
    } == {
        "final_response": "overrides applied",
        "request_model": "mock-model-override",
        "usage": {
            "last": {
                "cachedInputTokens": 3,
                "inputTokens": 11,
                "outputTokens": 7,
                "reasoningOutputTokens": 5,
                "totalTokens": 18,
            },
            "total": {
                "cachedInputTokens": 3,
                "inputTokens": 11,
                "outputTokens": 7,
                "reasoningOutputTokens": 5,
                "totalTokens": 18,
            },
        },
    }


def test_async_thread_run_uses_mock_responses(
    tmp_path,
) -> None:
    """Async Thread.run should exercise the same app-server boundary."""

    async def scenario() -> None:
        """Run the async client against a real app-server process."""
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_assistant_message(
                "Hello async.",
                response_id="async-run-1",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start()
                result = await thread.run("async hello")

            request = harness.responses.single_request()

        assert {
            "final_response": result.final_response,
            "agent_messages": agent_message_texts_from_items(result.items),
            "request_user_texts": request.message_input_texts("user")[-1:],
        } == {
            "final_response": "Hello async.",
            "agent_messages": ["Hello async."],
            "request_user_texts": ["async hello"],
        }

    asyncio.run(scenario())


def test_sync_run_result_uses_last_unknown_phase_message(tmp_path) -> None:
    """RunResult should use the last unknown-phase agent message as final text."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("items-last"),
                    ev_assistant_message("msg-items-first", "First message"),
                    ev_assistant_message("msg-items-second", "Second message"),
                    ev_completed("items-last"),
                ]
            )
        )

        with Codex(config=harness.app_server_config()) as codex:
            result = codex.thread_start().run("case: last unknown phase wins")

    assert {
        "final_response": result.final_response,
        "agent_messages": agent_message_texts_from_items(result.items),
    } == {
        "final_response": "Second message",
        "agent_messages": ["First message", "Second message"],
    }


def test_sync_run_result_preserves_empty_last_message(tmp_path) -> None:
    """RunResult should preserve an empty final agent message instead of skipping it."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("items-empty"),
                    ev_assistant_message("msg-items-nonempty", "First message"),
                    ev_assistant_message("msg-items-empty", ""),
                    ev_completed("items-empty"),
                ]
            )
        )

        with Codex(config=harness.app_server_config()) as codex:
            result = codex.thread_start().run("case: empty last message")

    assert {
        "final_response": result.final_response,
        "agent_messages": agent_message_texts_from_items(result.items),
    } == {
        "final_response": "",
        "agent_messages": ["First message", ""],
    }


def test_sync_run_result_does_not_promote_commentary_only_to_final(tmp_path) -> None:
    """RunResult final_response should stay unset when app-server marks only commentary."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("items-commentary"),
                    assistant_message_with_phase(
                        "msg-items-commentary",
                        "Commentary",
                        MessagePhase.commentary,
                    ),
                    ev_completed("items-commentary"),
                ]
            )
        )

        with Codex(config=harness.app_server_config()) as codex:
            result = codex.thread_start().run("case: commentary only")

    assert {
        "final_response": result.final_response,
        "agent_messages": agent_message_texts_from_items(result.items),
    } == {
        "final_response": None,
        "agent_messages": ["Commentary"],
    }


def test_async_run_result_uses_last_unknown_phase_message(tmp_path) -> None:
    """Async RunResult should use the last unknown-phase agent message."""

    async def scenario() -> None:
        """Run one async result-mapping case against a pinned app-server."""
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_sse(
                sse(
                    [
                        ev_response_created("async-items-last"),
                        ev_assistant_message(
                            "msg-async-items-first",
                            "First async message",
                        ),
                        ev_assistant_message(
                            "msg-async-items-second",
                            "Second async message",
                        ),
                        ev_completed("async-items-last"),
                    ]
                )
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                result = await (await codex.thread_start()).run("case: async last unknown phase")

        assert {
            "final_response": result.final_response,
            "agent_messages": agent_message_texts_from_items(result.items),
        } == {
            "final_response": "Second async message",
            "agent_messages": ["First async message", "Second async message"],
        }

    asyncio.run(scenario())


def test_async_run_result_does_not_promote_commentary_only_to_final(
    tmp_path,
) -> None:
    """Async RunResult final_response should stay unset for commentary-only output."""

    async def scenario() -> None:
        """Run one async commentary mapping case against a pinned app-server."""
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_sse(
                sse(
                    [
                        ev_response_created("async-items-commentary"),
                        assistant_message_with_phase(
                            "msg-async-items-commentary",
                            "Async commentary",
                            MessagePhase.commentary,
                        ),
                        ev_completed("async-items-commentary"),
                    ]
                )
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                result = await (await codex.thread_start()).run("case: async commentary only")

        assert {
            "final_response": result.final_response,
            "agent_messages": agent_message_texts_from_items(result.items),
        } == {
            "final_response": None,
            "agent_messages": ["Async commentary"],
        }

    asyncio.run(scenario())


def test_thread_run_raises_when_real_app_server_reports_failed_turn(tmp_path) -> None:
    """Thread.run should surface the failed turn error emitted by app-server."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("failed-run"),
                    ev_failed("failed-run", "boom from mock model"),
                ]
            )
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            with pytest.raises(RuntimeError, match="boom from mock model"):
                thread.run("trigger failure")


def test_final_answer_phase_survives_real_app_server_mapping(tmp_path) -> None:
    """RunResult should use the final-answer item emitted by app-server."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("phase-1"),
                    {
                        **ev_assistant_message("msg-commentary", "Commentary"),
                        "item": {
                            **ev_assistant_message("msg-commentary", "Commentary")["item"],
                            "phase": MessagePhase.commentary.value,
                        },
                    },
                    {
                        **ev_assistant_message("msg-final", "Final answer"),
                        "item": {
                            **ev_assistant_message("msg-final", "Final answer")["item"],
                            "phase": MessagePhase.final_answer.value,
                        },
                    },
                    ev_completed("phase-1"),
                ]
            )
        )

        with Codex(config=harness.app_server_config()) as codex:
            result = codex.thread_start().run("choose final answer")

    assert {
        "final_response": result.final_response,
        "items": [
            {
                "text": item.root.text,
                "phase": None if item.root.phase is None else item.root.phase.value,
            }
            for item in result.items
            if item.root.type == "agentMessage"
        ],
    } == {
        "final_response": "Final answer",
        "items": [
            {"text": "Commentary", "phase": MessagePhase.commentary.value},
            {"text": "Final answer", "phase": MessagePhase.final_answer.value},
        ],
    }
