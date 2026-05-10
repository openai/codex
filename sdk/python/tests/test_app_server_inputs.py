from __future__ import annotations

from app_server_harness import AppServerHarness
from openai_codex import Codex, ImageInput, LocalImageInput, SkillInput, TextInput
from app_server_helpers import TINY_PNG_BYTES


def _history_input_summary(read_response) -> list[tuple[str, str, str | None]]:
    """Return text and skill inputs persisted in a read thread history."""
    summary: list[tuple[str, str, str | None]] = []
    for turn in read_response.thread.turns:
        for item in turn.items:
            root = item.root
            if root.type != "userMessage":
                continue
            for input_item in root.content:
                input_root = input_item.root
                if input_root.type == "text":
                    summary.append(("text", input_root.text, None))
                if input_root.type == "skill":
                    summary.append(("skill", input_root.name, input_root.path))
    return summary


def test_remote_image_input_reaches_responses_api(
    tmp_path,
) -> None:
    """Remote image inputs should survive the SDK and app-server boundary."""
    remote_image_url = "https://example.com/codex.png"

    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message(
            "remote image received",
            response_id="remote-image",
        )

        with Codex(config=harness.app_server_config()) as codex:
            result = codex.thread_start().run(
                [
                    TextInput("Describe the remote image."),
                    ImageInput(remote_image_url),
                ]
            )
            request = harness.responses.single_request()

    assert {
        "final_response": result.final_response,
        "contains_user_prompt": "Describe the remote image."
        in request.message_input_texts("user"),
        "image_urls": request.message_image_urls("user"),
    } == {
        "final_response": "remote image received",
        "contains_user_prompt": True,
        "image_urls": [remote_image_url],
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
        "contains_user_prompt": "Describe the local image."
        in request.message_input_texts("user"),
        "image_url_is_png_data_url": request.message_image_urls("user")[-1].startswith(
            "data:image/png;base64,"
        ),
    } == {
        "final_response": "local image received",
        "contains_user_prompt": True,
        "image_url_is_png_data_url": True,
    }


def test_skill_input_is_persisted_in_thread_history(tmp_path) -> None:
    """SkillInput should cross the SDK boundary as structured user input."""
    skill_file = tmp_path / "skills" / "demo" / "SKILL.md"
    skill_file.parent.mkdir(parents=True)
    skill_file.write_text("# Demo\n\nUse the word cobalt.\n")

    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message(
            "skill received",
            response_id="skill-input",
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            result = thread.run(
                [
                    TextInput("Use the selected skill."),
                    SkillInput("demo", str(skill_file)),
                ]
            )
            read = thread.read(include_turns=True)

    assert {
        "final_response": result.final_response,
        "history_inputs": _history_input_summary(read),
    } == {
        "final_response": "skill received",
        "history_inputs": [
            ("text", "Use the selected skill.", None),
            ("skill", "demo", str(skill_file)),
        ],
    }
