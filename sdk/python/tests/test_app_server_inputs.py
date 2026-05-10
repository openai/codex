from __future__ import annotations

from app_server_harness import AppServerHarness
from openai_codex import Codex, ImageInput, LocalImageInput, SkillInput, TextInput
from app_server_helpers import TINY_PNG_BYTES


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


def test_skill_input_injects_loaded_skill_body(tmp_path) -> None:
    """SkillInput should inject the selected loaded skill into model input."""
    skill_body = "Use the word cobalt."

    with AppServerHarness(tmp_path) as harness:
        skill_file = harness.workspace / ".agents" / "skills" / "demo" / "SKILL.md"
        skill_file.parent.mkdir(parents=True)
        skill_file.write_text(
            f"---\nname: demo\ndescription: demo skill\n---\n\n{skill_body}\n"
        )
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
        text
        for text in request.message_input_texts("user")
        if text.startswith("<skill>")
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
