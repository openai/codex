from __future__ import annotations

import asyncio

import pytest

from openai_codex_sdk.exec import CodexExec, CodexExecArgs, serialize_config_overrides


class FakeStdin:
    def __init__(self) -> None:
        self.data = bytearray()
        self.closed = False

    def write(self, value: bytes) -> None:
        self.data.extend(value)

    async def drain(self) -> None:
        return None

    def close(self) -> None:
        self.closed = True

    async def wait_closed(self) -> None:
        return None


class FakeProcess:
    def __init__(
        self,
        stdout_lines: list[str],
        stderr_text: str = "",
        returncode: int = 0,
    ) -> None:
        self.stdin = FakeStdin()
        self.stdout = asyncio.StreamReader()
        for line in stdout_lines:
            self.stdout.feed_data(line.encode("utf-8") + b"\n")
        self.stdout.feed_eof()

        self.stderr = asyncio.StreamReader()
        if stderr_text:
            self.stderr.feed_data(stderr_text.encode("utf-8"))
        self.stderr.feed_eof()

        self._final_returncode = returncode
        self.returncode: int | None = None
        self.terminated = False
        self.killed = False

    async def wait(self) -> int:
        if self.returncode is None:
            self.returncode = self._final_returncode
        return self.returncode

    def terminate(self) -> None:
        self.terminated = True
        if self.returncode is None:
            self.returncode = -15

    def kill(self) -> None:
        self.killed = True
        if self.returncode is None:
            self.returncode = -9


@pytest.mark.asyncio
async def test_exec_builds_expected_args_and_env(monkeypatch: pytest.MonkeyPatch) -> None:
    captured: dict[str, object] = {}

    async def fake_create_subprocess_exec(*cmd, **kwargs):  # noqa: ANN001
        captured["cmd"] = cmd
        captured["kwargs"] = kwargs
        return FakeProcess(
            [
                '{"type":"thread.started","thread_id":"thread_1"}',
                '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1}}',
            ]
        )

    monkeypatch.setattr(asyncio, "create_subprocess_exec", fake_create_subprocess_exec)

    exec_client = CodexExec(
        executable_path="codex-bin",
        env={"CUSTOM_ENV": "custom"},
        config_overrides={"approval_policy": "never"},
    )
    args = CodexExecArgs(
        input="hello",
        base_url="https://api.example.com",
        api_key="test-key",
        thread_id="thread_1",
        images=["/tmp/a.png"],
        model="gpt-test",
        sandbox_mode="workspace-write",
        working_directory="/tmp/repo",
        additional_directories=["/tmp/shared-a", "/tmp/shared-b"],
        skip_git_repo_check=True,
        output_schema_file="/tmp/schema.json",
        model_reasoning_effort="high",
        network_access_enabled=True,
        web_search_mode="cached",
        approval_policy="on-request",
    )

    lines = [line async for line in exec_client.run(args)]

    assert len(lines) == 2
    cmd = list(captured["cmd"])  # type: ignore[arg-type]
    assert cmd[0] == "codex-bin"
    arg_list = cmd[1:]

    assert "--experimental-json" in arg_list
    assert "--model" in arg_list
    assert "--sandbox" in arg_list
    assert "--cd" in arg_list
    assert "--skip-git-repo-check" in arg_list
    assert "--output-schema" in arg_list
    assert "resume" in arg_list
    assert "--image" in arg_list

    resume_index = arg_list.index("resume")
    image_index = arg_list.index("--image")
    assert resume_index < image_index

    kwargs = captured["kwargs"]  # type: ignore[assignment]
    env = kwargs["env"]  # type: ignore[index]
    assert env["CUSTOM_ENV"] == "custom"  # type: ignore[index]
    assert env["OPENAI_BASE_URL"] == "https://api.example.com"  # type: ignore[index]
    assert env["CODEX_API_KEY"] == "test-key"  # type: ignore[index]
    assert env["CODEX_INTERNAL_ORIGINATOR_OVERRIDE"] == "codex_sdk_py"  # type: ignore[index]


@pytest.mark.asyncio
async def test_exec_raises_on_nonzero_exit(monkeypatch: pytest.MonkeyPatch) -> None:
    async def fake_create_subprocess_exec(*_cmd, **_kwargs):  # noqa: ANN001
        return FakeProcess(stdout_lines=[], stderr_text="boom", returncode=2)

    monkeypatch.setattr(asyncio, "create_subprocess_exec", fake_create_subprocess_exec)

    exec_client = CodexExec(executable_path="codex-bin")
    with pytest.raises(RuntimeError, match="Codex Exec exited with code 2: boom"):
        async for _ in exec_client.run(CodexExecArgs(input="hello")):
            pass


def test_serialize_config_overrides() -> None:
    overrides = serialize_config_overrides(
        {
            "approval_policy": "never",
            "sandbox_workspace_write": {"network_access": True},
            "retry_budget": 3,
            "tool_rules": {"allow": ["git status", "git diff"]},
        }
    )

    assert 'approval_policy="never"' in overrides
    assert "sandbox_workspace_write.network_access=true" in overrides
    assert "retry_budget=3" in overrides
    assert 'tool_rules.allow=["git status", "git diff"]' in overrides
