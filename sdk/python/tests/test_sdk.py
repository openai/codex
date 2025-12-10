import json
import os
import subprocess
import tempfile
import threading
import time
import unittest
import unittest.mock
from contextlib import contextmanager
from pathlib import Path
from typing import List, Tuple

from codex_sdk import (
    Codex,
    CodexOptions,
    ThreadOptions,
    ThreadRunError,
    TurnOptions,
)
from codex_sdk.exec import INTERNAL_ORIGINATOR_ENV
from codex_sdk.types import ThreadEvent

from .responses_proxy import (
    assistant_message,
    response_completed,
    response_failed,
    response_started,
    shell_call,
    sse,
    start_responses_test_proxy,
)

CODEX_EXEC_PATH = Path(__file__).resolve().parents[2] / "codex-rs" / "target" / "debug" / "codex"


def expect_pair(args: List[str], flag: str, value: str) -> None:
    self_index = args.index(flag)
    assert args[self_index + 1] == value


@contextmanager
def spy_popen() -> Tuple[list, list]:
    real_popen = subprocess.Popen
    calls: list = []
    envs: list = []

    def wrapper(*args, **kwargs):
        calls.append(list(args[0]))
        envs.append(kwargs.get("env"))
        return real_popen(*args, **kwargs)

    with unittest.mock.patch("codex_sdk.exec.subprocess.Popen", side_effect=wrapper):
        yield calls, envs


class CodexSdkTests(unittest.TestCase):
    def _start_proxy(self, bodies):
        try:
            return start_responses_test_proxy(bodies)
        except RuntimeError as exc:
            self.skipTest(str(exc))

    def test_returns_thread_events(self) -> None:
        url, requests, close = self._start_proxy([sse(response_started(), assistant_message("Hi!"), response_completed())])
        try:
            client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
            thread = client.start_thread()
            result = thread.run("Hello, world!")

            self.assertEqual(result.final_response, "Hi!")
            self.assertEqual(len(result.items), 1)
            self.assertIsNotNone(result.usage)
            self.assertIsNotNone(thread.id)
            self.assertGreater(len(requests), 0)
        finally:
            close()

    def test_run_twice_continues_thread(self) -> None:
        url, requests, close = self._start_proxy(
            [
                sse(response_started("response_1"), assistant_message("First response", "item_1"), response_completed("response_1")),
                sse(response_started("response_2"), assistant_message("Second response", "item_2"), response_completed("response_2")),
            ]
        )
        try:
            client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
            thread = client.start_thread()
            thread.run("first input")
            thread.run("second input")

            second_request = requests[1]
            payload = second_request.json
            assistant_entry = next((entry for entry in payload["input"] if entry.get("role") == "assistant"), None)
            self.assertIsNotNone(assistant_entry)
            content = assistant_entry.get("content") or []
            assistant_text = next((item.get("text") for item in content if item.get("type") == "output_text"), None)
            self.assertEqual(assistant_text, "First response")
        finally:
            close()

    def test_resume_thread_by_id(self) -> None:
        url, requests, close = self._start_proxy(
            [
                sse(response_started("response_1"), assistant_message("First response", "item_1"), response_completed("response_1")),
                sse(response_started("response_2"), assistant_message("Second response", "item_2"), response_completed("response_2")),
            ]
        )
        try:
            client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
            original = client.start_thread()
            original.run("first input")

            resumed = client.resume_thread(original.id or "")
            result = resumed.run("second input")

            self.assertEqual(resumed.id, original.id)
            self.assertEqual(result.final_response, "Second response")

            second_request = requests[1]
            payload = second_request.json
            assistant_entry = next((entry for entry in payload["input"] if entry.get("role") == "assistant"), None)
            content = assistant_entry.get("content") if assistant_entry else []
            assistant_text = next((item.get("text") for item in content if item.get("type") == "output_text"), None)
            self.assertEqual(assistant_text, "First response")
        finally:
            close()

    def test_run_streamed(self) -> None:
        url, requests, close = self._start_proxy(
            [sse(response_started(), assistant_message("Hi!"), response_completed())]
        )
        try:
            client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
            thread = client.start_thread()
            streamed = thread.run_streamed("Hello, world!")

            events: List[ThreadEvent] = list(streamed.events)
            self.assertEqual(len(events), 4)
            self.assertIsNotNone(thread.id)
            self.assertGreater(len(requests), 0)
        finally:
            close()

    def test_thread_options_passed_to_exec(self) -> None:
        url, requests, close = self._start_proxy(
            [sse(response_started("response_1"), assistant_message("Turn options applied", "item_1"), response_completed("response_1"))]
        )
        with spy_popen() as (calls, _envs):
            try:
                client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
                thread = client.start_thread(ThreadOptions(model="gpt-test-1", sandbox_mode="workspace-write"))
                thread.run("apply options")
                command_args = calls[0]
                self.assertIn("--sandbox", command_args)
                self.assertIn("workspace-write", command_args)
                self.assertIn("--model", command_args)
                self.assertIn("gpt-test-1", command_args)
            finally:
                close()

    def test_model_reasoning_effort_flag(self) -> None:
        url, _requests, close = self._start_proxy(
            [sse(response_started("response_1"), assistant_message("Reasoning effort applied", "item_1"), response_completed("response_1"))]
        )
        with spy_popen() as (calls, _envs):
            try:
                client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
                thread = client.start_thread(ThreadOptions(model_reasoning_effort="high"))
                thread.run("apply reasoning effort")
                command_args = calls[0]
                expect_pair(command_args, "--config", 'model_reasoning_effort="high"')
            finally:
                close()

    def test_network_access_flag(self) -> None:
        url, _requests, close = self._start_proxy(
            [sse(response_started("response_1"), assistant_message("Network access enabled", "item_1"), response_completed("response_1"))]
        )
        with spy_popen() as (calls, _envs):
            try:
                client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
                thread = client.start_thread(ThreadOptions(network_access_enabled=True))
                thread.run("test network access")
                command_args = calls[0]
                expect_pair(command_args, "--config", "sandbox_workspace_write.network_access=true")
            finally:
                close()

    def test_web_search_flag(self) -> None:
        url, _requests, close = self._start_proxy(
            [sse(response_started("response_1"), assistant_message("Web search enabled", "item_1"), response_completed("response_1"))]
        )
        with spy_popen() as (calls, _envs):
            try:
                client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
                thread = client.start_thread(ThreadOptions(web_search_enabled=True))
                thread.run("test web search")
                command_args = calls[0]
                expect_pair(command_args, "--config", "features.web_search_request=true")
            finally:
                close()

    def test_approval_policy_flag(self) -> None:
        url, _requests, close = self._start_proxy(
            [sse(response_started("response_1"), assistant_message("Approval policy set", "item_1"), response_completed("response_1"))]
        )
        with spy_popen() as (calls, _envs):
            try:
                client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
                thread = client.start_thread(ThreadOptions(approval_policy="on-request"))
                thread.run("test approval policy")
                command_args = calls[0]
                expect_pair(command_args, "--config", 'approval_policy="on-request"')
            finally:
                close()

    def test_env_override(self) -> None:
        url, _requests, close = self._start_proxy(
            [sse(response_started("response_1"), assistant_message("Custom env", "item_1"), response_completed("response_1"))]
        )
        os.environ["CODEX_ENV_SHOULD_NOT_LEAK"] = "leak"
        with spy_popen() as (_calls, envs):
            try:
                client = Codex(
                    CodexOptions(
                        codex_path_override=str(CODEX_EXEC_PATH),
                        base_url=url,
                        api_key="test",
                        env={"CUSTOM_ENV": "custom"},
                    )
                )
                thread = client.start_thread()
                thread.run("custom env")

                spawn_env = envs[0]
                self.assertIsNotNone(spawn_env)
                if spawn_env:
                    self.assertEqual(spawn_env.get("CUSTOM_ENV"), "custom")
                    self.assertIsNone(spawn_env.get("CODEX_ENV_SHOULD_NOT_LEAK"))
                    self.assertEqual(spawn_env.get("OPENAI_BASE_URL"), url)
                    self.assertEqual(spawn_env.get("CODEX_API_KEY"), "test")
                    self.assertEqual(spawn_env.get(INTERNAL_ORIGINATOR_ENV), "codex_sdk_py")
            finally:
                os.environ.pop("CODEX_ENV_SHOULD_NOT_LEAK", None)
                close()

    def test_additional_directories(self) -> None:
        url, _requests, close = self._start_proxy(
            [sse(response_started("response_1"), assistant_message("Additional directories applied", "item_1"), response_completed("response_1"))]
        )
        with spy_popen() as (calls, _envs):
            try:
                client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
                thread = client.start_thread(ThreadOptions(additional_directories=["../backend", "/tmp/shared"]))
                thread.run("test additional dirs")
                command_args = calls[0]
                forwarded = []
                for index, arg in enumerate(command_args):
                    if arg == "--add-dir" and index + 1 < len(command_args):
                        forwarded.append(command_args[index + 1])
                self.assertEqual(forwarded, ["../backend", "/tmp/shared"])
            finally:
                close()

    def test_output_schema_written_and_cleaned(self) -> None:
        url, requests, close = self._start_proxy(
            [sse(response_started("response_1"), assistant_message("Structured response", "item_1"), response_completed("response_1"))]
        )
        schema = {
            "type": "object",
            "properties": {"answer": {"type": "string"}},
            "required": ["answer"],
            "additionalProperties": False,
        }
        with spy_popen() as (calls, _envs):
            try:
                client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
                thread = client.start_thread()
                thread.run("structured", TurnOptions(output_schema=schema))

                payload = requests[0].json
                text = payload.get("text")
                self.assertIsNotNone(text)
                if text:
                    self.assertEqual(
                        text.get("format"),
                        {"name": "codex_output_schema", "type": "json_schema", "strict": True, "schema": schema},
                    )

                command_args = calls[0]
                schema_index = command_args.index("--output-schema")
                schema_path = command_args[schema_index + 1]
                self.assertFalse(Path(schema_path).exists())
            finally:
                close()

    def test_combines_text_segments(self) -> None:
        url, requests, close = self._start_proxy(
            [sse(response_started("response_1"), assistant_message("Combined input applied", "item_1"), response_completed("response_1"))]
        )
        try:
            client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
            thread = client.start_thread()
            thread.run(
                [
                    {"type": "text", "text": "Describe file changes"},
                    {"type": "text", "text": "Focus on impacted tests"},
                ]
            )

            payload = requests[0].json
            last_user = payload["input"][-1]
            text = last_user["content"][0]["text"]
            self.assertEqual(text, "Describe file changes\n\nFocus on impacted tests")
        finally:
            close()

    def test_forwards_images(self) -> None:
        url, _requests, close = self._start_proxy(
            [sse(response_started("response_1"), assistant_message("Images applied", "item_1"), response_completed("response_1"))]
        )
        temp_dir = tempfile.mkdtemp(prefix="codex-images-")
        images = [str(Path(temp_dir) / "first.png"), str(Path(temp_dir) / "second.jpg")]
        for index, image_path in enumerate(images):
            Path(image_path).write_text(f"image-{index}", encoding="utf-8")
        with spy_popen() as (calls, _envs):
            try:
                client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
                thread = client.start_thread()
                thread.run(
                    [
                        {"type": "text", "text": "describe the images"},
                        {"type": "local_image", "path": images[0]},
                        {"type": "local_image", "path": images[1]},
                    ]
                )
                command_args = calls[0]
                forwarded = []
                for index, arg in enumerate(command_args):
                    if arg == "--image" and index + 1 < len(command_args):
                        forwarded.append(command_args[index + 1])
                self.assertEqual(forwarded, images)
            finally:
                close()
                try:
                    for image in images:
                        Path(image).unlink(missing_ok=True)
                    Path(temp_dir).rmdir()
                except Exception:
                    pass

    def test_working_directory(self) -> None:
        url, _requests, close = self._start_proxy(
            [sse(response_started("response_1"), assistant_message("Working directory applied", "item_1"), response_completed("response_1"))]
        )
        with spy_popen() as (calls, _envs):
            try:
                working_dir = tempfile.mkdtemp(prefix="codex-working-dir-")
                client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
                thread = client.start_thread(ThreadOptions(working_directory=working_dir, skip_git_repo_check=True))
                thread.run("use custom working directory")
                command_args = calls[0]
                expect_pair(command_args, "--cd", working_dir)
            finally:
                close()
                try:
                    Path(working_dir).rmdir()
                except Exception:
                    pass

    def test_working_directory_without_git_check_fails(self) -> None:
        url, _requests, close = self._start_proxy(
            [sse(response_started("response_1"), assistant_message("Working directory applied", "item_1"), response_completed("response_1"))]
        )
        try:
            working_dir = tempfile.mkdtemp(prefix="codex-working-dir-")
            client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
            thread = client.start_thread(ThreadOptions(working_directory=working_dir))
            with self.assertRaises(ThreadRunError):
                thread.run("use custom working directory")
        finally:
            close()
            try:
                Path(working_dir).rmdir()
            except Exception:
                pass

    def test_originator_header(self) -> None:
        url, requests, close = self._start_proxy(
            [sse(response_started(), assistant_message("Hi!"), response_completed())]
        )
        try:
            client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
            thread = client.start_thread()
            thread.run("Hello, originator!")

            originator = requests[0].headers.get("originator")
            self.assertIn("codex_sdk_py", originator)
        finally:
            close()

    def test_turn_failure_raises(self) -> None:
        def failure_events():
            yield sse(response_started("response_1"))
            while True:
                yield sse(response_failed("rate limit exceeded"))

        url, _requests, close = self._start_proxy(failure_events())
        try:
            client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
            thread = client.start_thread()
            with self.assertRaises(ThreadRunError):
                thread.run("fail")
        finally:
            close()

    def test_cancellation_before_start(self) -> None:
        url, _requests, close = self._start_proxy(
            [sse(response_started(), shell_call(), response_completed())]
        )
        cancel_event = threading.Event()
        cancel_event.set()
        try:
            client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
            thread = client.start_thread()
            with self.assertRaises(Exception):
                thread.run("Hello, world!", TurnOptions(cancellation_event=cancel_event))
        finally:
            close()

    def test_cancellation_during_iteration(self) -> None:
        def endless_shell_calls():
            while True:
                yield sse(response_started(), shell_call(), response_completed())

        url, _requests, close = self._start_proxy(endless_shell_calls())
        cancel_event = threading.Event()
        try:
            client = Codex(CodexOptions(codex_path_override=str(CODEX_EXEC_PATH), base_url=url, api_key="test"))
            thread = client.start_thread()
            turn = thread.run_streamed("Hello, world!", TurnOptions(cancellation_event=cancel_event))

            def cancel_soon():
                time.sleep(0.05)
                cancel_event.set()

            threading.Thread(target=cancel_soon, daemon=True).start()
            with self.assertRaises(Exception):
                for _event in turn.events:
                    pass
        finally:
            close()
