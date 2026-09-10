from __future__ import annotations

import asyncio
import base64
from concurrent.futures import ThreadPoolExecutor

from app_server_harness import AppServerHarness
from app_server_helpers import TINY_PNG_BYTES, streaming_response

from openai_codex import (
    AsyncCodex,
    Codex,
    ExternalMessage,
    ImageInput,
    LocalImageInput,
    SkillInput,
    TextInput,
)


def _external_items(request) -> list[dict]:
    """Select model-visible external content without generated item identifiers."""
    return [
        {key: value for key, value in item.items() if key != "id"}
        for item in request.input()
        if item.get("type") == "function_call_output"
    ]


def test_external_message_preserves_tool_authority_through_resume(tmp_path) -> None:
    content = "External update: deployment completed."
    expected = {
        "type": "function_call_output",
        "name": "notifications",
        "namespace": "slack",
        "output": content,
    }
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("Update received", response_id="external")
        harness.responses.enqueue_assistant_message("Still available", response_id="resumed")
        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            result = thread.run(
                ExternalMessage(tool_name="notifications", namespace="slack", content=content)
            )
            external_item = next(
                item for item in result.items if item.root.type == "functionCallOutput"
            )
        with Codex(config=harness.app_server_config()) as codex:
            resumed = codex.thread_resume(thread.id, include_turns=False)
            history = resumed.read(include_turns=True)
            assert external_item in history.thread.turns[0].items
            resumed.run("Summarize the external update.")
        requests = harness.responses.requests()

    assert result.final_response == "Update received"
    assert [_external_items(request) for request in requests] == [[expected], [expected]]
    assert [
        text
        for request in requests
        for role in ("user", "developer")
        for text in request.message_input_texts(role)
        if content in text
    ] == []


def test_external_message_joins_active_turn_with_tool_authority(tmp_path) -> None:
    content = "External update while the agent is running."
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_sse(
            streaming_response("external-first", "msg-first", ["Working"]),
            delay_between_events_s=0.2,
        )
        harness.responses.enqueue_assistant_message(
            "Update processed", response_id="external-second"
        )
        with ThreadPoolExecutor(max_workers=2) as consumers:
            with Codex(config=harness.app_server_config()) as codex:
                thread = codex.thread_start()
                original = thread.turn("Monitor deployment updates.")
                original_result = consumers.submit(original.run)
                harness.responses.wait_for_requests(1)
                joined = thread.turn(ExternalMessage(tool_name="notifications", content=content))
                result = consumers.submit(joined.run).result(timeout=15)
                first = original_result.result(timeout=15)
                assert first.final_response == result.final_response
                assert first.items[0].root.type == "userMessage"
                assert all(item.root.type != "userMessage" for item in result.items)
                assert codex._client._router._turn_states == {}
        requests = harness.responses.requests()

    assert result.usage is not None
    assert (joined.id, result.final_response) == (original.id, "Update processed")
    assert [_external_items(request) for request in requests] == [
        [],
        [
            {
                "type": "function_call_output",
                "name": "notifications",
                "output": content,
            }
        ],
    ]
    assert [request.message_input_texts("user")[-1] for request in requests] == [
        "Monitor deployment updates.",
        "Monitor deployment updates.",
    ]


def test_async_external_message_reaches_model_with_tool_authority(tmp_path) -> None:
    async def scenario() -> None:
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_assistant_message(
                "Async update received", response_id="external-async"
            )
            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start()
                result = await thread.run(
                    ExternalMessage(tool_name="notifications", content="External async update")
                )
            request = harness.responses.single_request()

        assert result.final_response == "Async update received"
        assert _external_items(request) == [
            {
                "type": "function_call_output",
                "name": "notifications",
                "output": "External async update",
            }
        ]
        assert "External async update" not in request.message_input_texts("user")

    asyncio.run(scenario())


def test_async_external_message_allows_both_handles_to_consume(tmp_path) -> None:
    async def scenario() -> None:
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_sse(
                streaming_response("async-first", "msg-first", ["Working"]),
                delay_between_events_s=0.2,
            )
            harness.responses.enqueue_assistant_message(
                "Update processed", response_id="async-second"
            )
            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start()
                original = await thread.turn("Monitor deployment updates.")
                original_result = asyncio.create_task(original.run())
                await asyncio.to_thread(harness.responses.wait_for_requests, 1)
                joined = await thread.turn(
                    ExternalMessage(tool_name="notifications", content="Update")
                )
                first, second = await asyncio.wait_for(
                    asyncio.gather(original_result, joined.run()), timeout=15
                )
                assert (first.final_response, first.usage) == (second.final_response, second.usage)
                assert first.final_response == "Update processed"
                assert first.usage is not None
                assert codex._client._sync._router._turn_states == {}

    asyncio.run(scenario())


def test_external_message_uses_core_tool_output_truncation(tmp_path) -> None:
    content = "External observation. " * 500
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message(
            "Context received", response_id="truncated-external"
        )
        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start(config={"tool_output_token_limit": 32})
            thread.run(ExternalMessage(tool_name="notifications", content=content))
        request = harness.responses.single_request()

    [item] = _external_items(request)
    assert (item["name"], item["type"]) == ("notifications", "function_call_output")
    assert len(item["output"]) < len(content)
    assert "truncated" in item["output"].lower()


def test_data_url_image_input_reaches_responses_api(
    tmp_path,
) -> None:
    """Data URL image inputs should survive the SDK and app-server boundary."""
    image_data_url = "data:image/png;base64," + base64.b64encode(TINY_PNG_BYTES).decode("ascii")

    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message(
            "data URL image received",
            response_id="data-url-image",
        )

        with Codex(config=harness.app_server_config()) as codex:
            result = codex.thread_start().run(
                [
                    TextInput("Describe the data URL image."),
                    ImageInput(image_data_url),
                ]
            )
            request = harness.responses.single_request()

    assert {
        "final_response": result.final_response,
        "contains_user_prompt": "Describe the data URL image."
        in request.message_input_texts("user"),
        "image_url_is_png_data_url": request.message_image_urls("user")[-1].startswith(
            "data:image/png;base64,"
        ),
    } == {
        "final_response": "data URL image received",
        "contains_user_prompt": True,
        "image_url_is_png_data_url": True,
    }


def test_local_image_input_reaches_responses_api(
    tmp_path,
) -> None:
    """Local image inputs should become data URLs after crossing the app-server."""
    local_image = tmp_path / "local.png"
    local_image.write_bytes(TINY_PNG_BYTES)

    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message(
            "local image received",
            response_id="local-image",
        )

        with Codex(config=harness.app_server_config()) as codex:
            result = codex.thread_start().run(
                [
                    TextInput("Describe the local image."),
                    LocalImageInput(str(local_image)),
                ]
            )
            request = harness.responses.single_request()

    assert {
        "final_response": result.final_response,
        "contains_user_prompt": "Describe the local image." in request.message_input_texts("user"),
        "image_url_is_png_data_url": request.message_image_urls("user")[-1].startswith(
            "data:image/png;base64,"
        ),
    } == {
        "final_response": "local image received",
        "contains_user_prompt": True,
        "image_url_is_png_data_url": True,
    }


def test_skill_input_injects_loaded_skill_body(tmp_path) -> None:
    """SkillInput should inject the selected loaded skill into model input."""
    skill_body = "Use the word cobalt."

    with AppServerHarness(tmp_path) as harness:
        skill_file = harness.workspace / ".agents" / "skills" / "demo" / "SKILL.md"
        skill_file.parent.mkdir(parents=True)
        skill_file.write_text(f"---\nname: demo\ndescription: demo skill\n---\n\n{skill_body}\n")
        skill_path = skill_file.resolve()
        harness.responses.enqueue_assistant_message(
            "skill received",
            response_id="skill-input",
        )

        with Codex(config=harness.app_server_config()) as codex:
            result = codex.thread_start().run(
                [
                    TextInput("Use the selected skill."),
                    SkillInput("demo", str(skill_path)),
                ]
            )
            request = harness.responses.single_request()

    skill_blocks = [
        text for text in request.message_input_texts("user") if text.startswith("<skill>")
    ]
    assert {
        "final_response": result.final_response,
        "skill_blocks": [
            {
                "has_name": "<name>demo</name>" in text,
                "has_path": f"<path>{skill_path}</path>" in text,
                "has_body": skill_body in text,
            }
            for text in skill_blocks
        ],
    } == {
        "final_response": "skill received",
        "skill_blocks": [
            {
                "has_name": True,
                "has_path": True,
                "has_body": True,
            }
        ],
    }
