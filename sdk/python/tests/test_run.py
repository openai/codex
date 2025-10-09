from __future__ import annotations

import pytest
from pathlib import Path

from pydantic import BaseModel

from codex import ThreadOptions, TurnOptions
from codex.config import SandboxMode
from codex.exec import PYTHON_SDK_ORIGINATOR
from codex.thread import ThreadRunResult
from codex.exceptions import ThreadRunError

from .helpers import (
    assistant_message,
    response_completed,
    response_failed,
    response_started,
    sse,
    start_responses_proxy,
)

def expect_pair(command: list[str], flag: str, expected: str) -> None:
    index = command.index(flag)
    assert command[index + 1] == expected


def test_returns_thread_events(codex_client) -> None:
    proxy = start_responses_proxy([sse(response_started(), assistant_message("Hi!"), response_completed())])
    try:
        client = codex_client(proxy.url)
        thread = client.start_thread()
        turn = thread.run("Hello, world!")

        assert isinstance(turn, ThreadRunResult)
        assert thread.id is not None
        assert turn.final_response == "Hi!"
        assert turn.usage is not None
        assert turn.usage.input_tokens == 42
        assert turn.items[0].type == "agent_message"
    finally:
        proxy.close()


def test_sends_previous_items_on_second_run(codex_client) -> None:
    proxy = start_responses_proxy(
        [
            sse(response_started("response_1"), assistant_message("First response", "item_1"), response_completed("response_1")),
            sse(response_started("response_2"), assistant_message("Second response", "item_2"), response_completed("response_2")),
        ]
    )
    try:
        client = codex_client(proxy.url)
        thread = client.start_thread()
        thread.run("first input")
        thread.run("second input")

        assert len(proxy.requests) >= 2
        second_request = proxy.requests[1]
        assistant_entry = next(entry for entry in second_request["json"]["input"] if entry["role"] == "assistant")
        assistant_text = next(content for content in assistant_entry["content"] if content["type"] == "output_text")
        assert assistant_text["text"] == "First response"
    finally:
        proxy.close()


def test_continues_thread_when_run_called_twice_with_options(codex_client) -> None:
    proxy = start_responses_proxy(
        [
            sse(response_started("response_1"), assistant_message("First response", "item_1"), response_completed("response_1")),
            sse(response_started("response_2"), assistant_message("Second response", "item_2"), response_completed("response_2")),
        ]
    )
    try:
        client = codex_client(proxy.url)
        thread = client.start_thread()
        thread.run("first input")
        thread.run("second input")

        second_request = proxy.requests[1]
        payload = second_request["json"]
        user_entry = payload["input"][-1]
        assert user_entry["role"] == "user"
        assert user_entry["content"][0]["text"] == "second input"
        assistant_entry = next(entry for entry in payload["input"] if entry["role"] == "assistant")
        assistant_text = next(content for content in assistant_entry["content"] if content["type"] == "output_text")
        assert assistant_text["text"] == "First response"
    finally:
        proxy.close()


def test_resumes_thread_by_id(codex_client) -> None:
    proxy = start_responses_proxy(
        [
            sse(response_started("response_1"), assistant_message("First response", "item_1"), response_completed("response_1")),
            sse(response_started("response_2"), assistant_message("Second response", "item_2"), response_completed("response_2")),
        ]
    )
    try:
        client = codex_client(proxy.url)
        original_thread = client.start_thread()
        original_thread.run("first input")

        assert original_thread.id is not None
        resumed_thread = client.resume_thread(original_thread.id)
        result = resumed_thread.run("second input")

        assert resumed_thread.id == original_thread.id
        assert result.final_response == "Second response"

        second_request = proxy.requests[1]
        assistant_entry = next(entry for entry in second_request["json"]["input"] if entry["role"] == "assistant")
        assistant_text = next(content for content in assistant_entry["content"] if content["type"] == "output_text")
        assert assistant_text["text"] == "First response"
    finally:
        proxy.close()


def test_thread_options_are_forwarded(codex_client, codex_exec_spy) -> None:
    proxy = start_responses_proxy([sse(response_started(), assistant_message("Options applied"), response_completed())])
    try:
        client = codex_client(proxy.url)
        thread = client.start_thread(ThreadOptions(model="gpt-test-1", sandbox_mode=SandboxMode.WORKSPACE_WRITE))
        thread.run("apply options")

        payload = proxy.requests[0]["json"]
        assert payload.get("model") == "gpt-test-1"

        command = codex_exec_spy[0]["command"]
        expect_pair(command, "--sandbox", "workspace-write")
        expect_pair(command, "--model", "gpt-test-1")
    finally:
        proxy.close()


def test_structured_output_writes_temp_file(codex_client, codex_exec_spy) -> None:
    proxy = start_responses_proxy([sse(response_started(), assistant_message("Structured"), response_completed())])
    schema = {
        "type": "object",
        "properties": {"answer": {"type": "string"}},
        "required": ["answer"],
        "additionalProperties": False,
    }
    try:
        client = codex_client(proxy.url)
        thread = client.start_thread()
        thread.run("structured", TurnOptions(output_schema=schema))

        payload = proxy.requests[0]["json"]
        assert payload["text"]["format"]["schema"] == schema

        command = codex_exec_spy[0]["command"]
        schema_flag_index = command.index("--output-schema")
        schema_path = Path(command[schema_flag_index + 1])
        assert not schema_path.exists()
    finally:
        proxy.close()


def test_structured_output_accepts_pydantic_model(codex_client, codex_exec_spy) -> None:
    proxy = start_responses_proxy([sse(response_started(), assistant_message("Structured"), response_completed())])

    class ResponseModel(BaseModel):
        answer: str

    try:
        client = codex_client(proxy.url)
        thread = client.start_thread()
        thread.run("structured", TurnOptions(output_schema=ResponseModel))

        payload = proxy.requests[0]["json"]
        schema = payload["text"]["format"]["schema"]
        assert schema["type"] == "object"
        assert schema["properties"]["answer"]["type"] == "string"

        command = codex_exec_spy[0]["command"]
        schema_flag_index = command.index("--output-schema")
        schema_path = Path(command[schema_flag_index + 1])
        assert not schema_path.exists()
    finally:
        proxy.close()


def test_sets_originator_header(codex_client) -> None:
    proxy = start_responses_proxy([sse(response_started(), assistant_message("Hi!"), response_completed())])
    try:
        client = codex_client(proxy.url)
        thread = client.start_thread()
        thread.run("Hello")

        headers = proxy.requests[0]["headers"]
        assert headers.get("originator") == PYTHON_SDK_ORIGINATOR
    finally:
        proxy.close()


def test_thread_run_error_on_failure(codex_client) -> None:
    proxy = start_responses_proxy([
        sse(response_started("resp_1")),
        sse(response_failed("rate limit exceeded")),
    ])
    try:
        client = codex_client(proxy.url)
        thread = client.start_thread()
        with pytest.raises(ThreadRunError):
            thread.run("fail")
    finally:
        proxy.close()


def test_runs_in_provided_working_directory(codex_client, codex_exec_spy, tmp_path) -> None:
    proxy = start_responses_proxy([sse(response_started(), assistant_message("Working dir applied", "item_1"), response_completed())])
    working_dir = tmp_path / "codex-working-dir"
    working_dir.mkdir()
    try:
        client = codex_client(proxy.url)
        thread = client.start_thread(ThreadOptions(working_directory=str(working_dir), skip_git_repo_check=True))
        thread.run("use custom working directory")

        command = codex_exec_spy[0]["command"]
        expect_pair(command, "--cd", str(working_dir))
        assert "--skip-git-repo-check" in command
    finally:
        proxy.close()


def test_requires_git_directory_unless_skipped(codex_client, tmp_path) -> None:
    proxy = start_responses_proxy([sse(response_started(), assistant_message("Working dir applied", "item_1"), response_completed())])
    working_dir = tmp_path / "codex-working-dir"
    working_dir.mkdir()
    try:
        client = codex_client(proxy.url)
        thread = client.start_thread(ThreadOptions(working_directory=str(working_dir)))
        with pytest.raises(Exception) as exc_info:
            thread.run("use custom working directory")
        assert "Not inside a trusted directory" in str(exc_info.value)
    finally:
        proxy.close()
