"""Tests for Codex SDK transport layer."""

import os
import uuid
from unittest.mock import AsyncMock, MagicMock, patch

import anyio
import pytest

from agent_sdk._internal.transport.subprocess_cli import SubprocessCLITransport
from agent_sdk.types import CodexAgentOptions

DEFAULT_CLI_PATH = "/usr/bin/codex"


def make_options(**kwargs: object) -> CodexAgentOptions:
    """Construct options using the standard CLI path unless overridden."""

    cli_path = kwargs.pop("cli_path", DEFAULT_CLI_PATH)
    return CodexAgentOptions(cli_path=cli_path, **kwargs)


class TestSubprocessCLITransport:
    """Test subprocess transport implementation."""

    def test_find_cli_not_found(self):
        """Test CLI not found error."""
        from agent_sdk._errors import CLINotFoundError

        with (
            patch("shutil.which", return_value=None),
            patch("pathlib.Path.exists", return_value=False),
            pytest.raises(CLINotFoundError) as exc_info,
        ):
            SubprocessCLITransport(prompt="test", options=CodexAgentOptions())

        assert "Codex Code not found" in str(exc_info.value)

    def test_build_command_basic(self):
        """Test building basic CLI command."""
        transport = SubprocessCLITransport(prompt="Hello", options=make_options())

        cmd = transport._build_command()
        assert cmd[0] == "/usr/bin/codex"
        assert "--output-format" in cmd
        assert "stream-json" in cmd
        assert "--print" in cmd
        assert "Hello" in cmd
        assert "--system-prompt" in cmd
        assert cmd[cmd.index("--system-prompt") + 1] == ""

    def test_cli_path_accepts_pathlib_path(self):
        """Test that cli_path accepts pathlib.Path objects."""
        from pathlib import Path

        path = Path("/usr/bin/codex")
        transport = SubprocessCLITransport(
            prompt="Hello",
            options=CodexAgentOptions(cli_path=path),
        )

        # Path object is converted to string, compare with str(path)
        assert transport._cli_path == str(path)

    def test_build_command_with_system_prompt_string(self):
        """Test building CLI command with system prompt as string."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(
                system_prompt="Be helpful",
            ),
        )

        cmd = transport._build_command()
        assert "--system-prompt" in cmd
        assert "Be helpful" in cmd

    def test_build_command_with_system_prompt_preset(self):
        """Test building CLI command with system prompt preset."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(
                system_prompt={"type": "preset", "preset": "codex"},
            ),
        )

        cmd = transport._build_command()
        assert "--system-prompt" not in cmd
        assert "--append-system-prompt" not in cmd

    def test_build_command_with_system_prompt_preset_and_append(self):
        """Test building CLI command with system prompt preset and append."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(
                system_prompt={
                    "type": "preset",
                    "preset": "codex",
                    "append": "Be concise.",
                },
            ),
        )

        cmd = transport._build_command()
        assert "--system-prompt" not in cmd
        assert "--append-system-prompt" in cmd
        assert "Be concise." in cmd

    def test_build_command_with_options(self):
        """Test building CLI command with options."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(
                allowed_tools=["Read", "Write"],
                disallowed_tools=["Bash"],
                model="test-model",
                permission_mode="acceptEdits",
                max_turns=5,
            ),
        )

        cmd = transport._build_command()
        assert "--allowedTools" in cmd
        assert "Read,Write" in cmd
        assert "--disallowedTools" in cmd
        assert "Bash" in cmd
        assert "--model" in cmd
        assert "test-model" in cmd
        assert "--permission-mode" in cmd
        assert "acceptEdits" in cmd
        assert "--max-turns" in cmd
        assert "5" in cmd

    def test_build_command_with_fallback_model(self):
        """Test building CLI command with fallback_model option."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(
                model="opus",
                fallback_model="sonnet",
            ),
        )

        cmd = transport._build_command()
        assert "--model" in cmd
        assert "opus" in cmd
        assert "--fallback-model" in cmd
        assert "sonnet" in cmd

    def test_build_command_with_max_thinking_tokens(self):
        """Test building CLI command with max_thinking_tokens option."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(max_thinking_tokens=5000),
        )

        cmd = transport._build_command()
        assert "--max-thinking-tokens" in cmd
        assert "5000" in cmd

    def test_build_command_with_add_dirs(self):
        """Test building CLI command with add_dirs option."""
        from pathlib import Path

        dir1 = "/path/to/dir1"
        dir2 = Path("/path/to/dir2")
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(add_dirs=[dir1, dir2]),
        )

        cmd = transport._build_command()

        # Check that both directories are in the command
        assert "--add-dir" in cmd
        add_dir_indices = [i for i, x in enumerate(cmd) if x == "--add-dir"]
        assert len(add_dir_indices) == 2

        # The directories should appear after --add-dir flags
        dirs_in_cmd = [cmd[i + 1] for i in add_dir_indices]
        assert dir1 in dirs_in_cmd
        assert str(dir2) in dirs_in_cmd

    def test_session_continuation(self):
        """Test session continuation options."""
        transport = SubprocessCLITransport(
            prompt="Continue from before",
            options=make_options(continue_conversation=True, resume="session-123"),
        )

        cmd = transport._build_command()
        assert "--continue" in cmd
        assert "--resume" in cmd
        assert "session-123" in cmd

    def test_connect_close(self):
        """Test connect and close lifecycle."""

        async def _test():
            with patch("anyio.open_process") as mock_exec:
                # Mock version check process
                mock_version_process = MagicMock()
                mock_version_process.stdout = MagicMock()
                mock_version_process.stdout.receive = AsyncMock(
                    return_value=b"2.0.0 (Codex)"
                )
                mock_version_process.terminate = MagicMock()
                mock_version_process.wait = AsyncMock()

                # Mock main process
                mock_process = MagicMock()
                mock_process.returncode = None
                mock_process.terminate = MagicMock()
                mock_process.wait = AsyncMock()
                mock_process.stdout = MagicMock()
                mock_process.stderr = MagicMock()

                # Mock stdin with aclose method
                mock_stdin = MagicMock()
                mock_stdin.aclose = AsyncMock()
                mock_process.stdin = mock_stdin

                # Return version process first, then main process
                mock_exec.side_effect = [mock_version_process, mock_process]

                transport = SubprocessCLITransport(
                    prompt="test",
                    options=make_options(),
                )

                await transport.connect()
                assert transport._process is not None
                assert transport.is_ready()

                await transport.close()
                mock_process.terminate.assert_called_once()

        anyio.run(_test)

    def test_read_messages(self):
        """Test reading messages from CLI output."""
        # This test is simplified to just test the transport creation
        # The full async stream handling is tested in integration tests
        transport = SubprocessCLITransport(prompt="test", options=make_options())

        # The transport now just provides raw message reading via read_messages()
        # So we just verify the transport can be created and basic structure is correct
        assert transport._prompt == "test"
        assert transport._cli_path == "/usr/bin/codex"

    def test_connect_with_nonexistent_cwd(self):
        """Test that connect raises CLIConnectionError when cwd doesn't exist."""
        from agent_sdk._errors import CLIConnectionError

        async def _test():
            transport = SubprocessCLITransport(
                prompt="test",
                options=make_options(cwd="/this/directory/does/not/exist"),
            )

            with pytest.raises(CLIConnectionError) as exc_info:
                await transport.connect()

            assert "/this/directory/does/not/exist" in str(exc_info.value)

        anyio.run(_test)

    def test_build_command_with_settings_file(self):
        """Test building CLI command with settings as file path."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(settings="/path/to/settings.json"),
        )

        cmd = transport._build_command()
        assert "--settings" in cmd
        assert "/path/to/settings.json" in cmd

    def test_build_command_with_settings_json(self):
        """Test building CLI command with settings as JSON object."""
        settings_json = '{"permissions": {"allow": ["Bash(ls:*)"]}}'
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(settings=settings_json),
        )

        cmd = transport._build_command()
        assert "--settings" in cmd
        assert settings_json in cmd

    def test_build_command_with_extra_args(self):
        """Test building CLI command with extra_args for future flags."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(
                extra_args={
                    "new-flag": "value",
                    "boolean-flag": None,
                    "another-option": "test-value",
                }
            ),
        )

        cmd = transport._build_command()
        cmd_str = " ".join(cmd)

        # Check flags with values
        assert "--new-flag value" in cmd_str
        assert "--another-option test-value" in cmd_str

        # Check boolean flag (no value)
        assert "--boolean-flag" in cmd
        # Make sure boolean flag doesn't have a value after it
        boolean_idx = cmd.index("--boolean-flag")
        # Either it's the last element or the next element is another flag
        assert boolean_idx == len(cmd) - 1 or cmd[boolean_idx + 1].startswith("--")

    def test_build_command_with_mcp_servers(self):
        """Test building CLI command with mcp_servers option."""
        import json

        mcp_servers = {
            "test-server": {
                "type": "stdio",
                "command": "/path/to/server",
                "args": ["--option", "value"],
            }
        }

        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(mcp_servers=mcp_servers),
        )

        cmd = transport._build_command()

        # Find the --mcp-config flag and its value
        assert "--mcp-config" in cmd
        mcp_idx = cmd.index("--mcp-config")
        mcp_config_value = cmd[mcp_idx + 1]

        # Parse the JSON and verify structure
        config = json.loads(mcp_config_value)
        assert "mcpServers" in config
        assert config["mcpServers"] == mcp_servers

    def test_build_command_with_mcp_servers_as_file_path(self):
        """Test building CLI command with mcp_servers as file path."""
        from pathlib import Path

        # Test with string path
        string_path = "/path/to/mcp-config.json"
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(mcp_servers=string_path),
        )

        cmd = transport._build_command()
        assert "--mcp-config" in cmd
        mcp_idx = cmd.index("--mcp-config")
        assert cmd[mcp_idx + 1] == string_path

        # Test with Path object
        path_obj = Path("/path/to/mcp-config.json")
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(mcp_servers=path_obj),
        )

        cmd = transport._build_command()
        assert "--mcp-config" in cmd
        mcp_idx = cmd.index("--mcp-config")
        # Path object gets converted to string, compare with str(path_obj)
        assert cmd[mcp_idx + 1] == str(path_obj)

    def test_build_command_with_mcp_servers_as_json_string(self):
        """Test building CLI command with mcp_servers as JSON string."""
        json_config = '{"mcpServers": {"server": {"type": "stdio", "command": "test"}}}'
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(mcp_servers=json_config),
        )

        cmd = transport._build_command()
        assert "--mcp-config" in cmd
        mcp_idx = cmd.index("--mcp-config")
        assert cmd[mcp_idx + 1] == json_config

    def test_env_vars_passed_to_subprocess(self):
        """Test that custom environment variables are passed to the subprocess."""

        async def _test():
            test_value = f"test-{uuid.uuid4().hex[:8]}"
            custom_env = {
                "MY_TEST_VAR": test_value,
            }

            options = make_options(env=custom_env)

            # Mock the subprocess to capture the env argument
            with patch(
                "anyio.open_process", new_callable=AsyncMock
            ) as mock_open_process:
                # Mock version check process
                mock_version_process = MagicMock()
                mock_version_process.stdout = MagicMock()
                mock_version_process.stdout.receive = AsyncMock(
                    return_value=b"2.0.0 (Codex)"
                )
                mock_version_process.terminate = MagicMock()
                mock_version_process.wait = AsyncMock()

                # Mock main process
                mock_process = MagicMock()
                mock_process.stdout = MagicMock()
                mock_stdin = MagicMock()
                mock_stdin.aclose = AsyncMock()  # Add async aclose method
                mock_process.stdin = mock_stdin
                mock_process.returncode = None

                # Return version process first, then main process
                mock_open_process.side_effect = [mock_version_process, mock_process]

                transport = SubprocessCLITransport(
                    prompt="test",
                    options=options,
                )

                await transport.connect()

                # Verify open_process was called twice (version check + main process)
                assert mock_open_process.call_count == 2

                # Check the second call (main process) for env vars
                second_call_kwargs = mock_open_process.call_args_list[1].kwargs
                assert "env" in second_call_kwargs
                env_passed = second_call_kwargs["env"]

                # Check that custom env var was passed
                assert env_passed["MY_TEST_VAR"] == test_value

                # Verify SDK identifier is present
                assert "CODEX_CODE_ENTRYPOINT" in env_passed
                assert env_passed["CODEX_CODE_ENTRYPOINT"] == "sdk-py"

                # Verify system env vars are also included with correct values
                if "PATH" in os.environ:
                    assert "PATH" in env_passed
                    assert env_passed["PATH"] == os.environ["PATH"]

        anyio.run(_test)

    def test_connect_as_different_user(self):
        """Test connect as different user."""

        async def _test():
            custom_user = "testuser"
            options = make_options(user=custom_user)

            # Mock the subprocess to capture the env argument
            with patch(
                "anyio.open_process", new_callable=AsyncMock
            ) as mock_open_process:
                # Mock version check process
                mock_version_process = MagicMock()
                mock_version_process.stdout = MagicMock()
                mock_version_process.stdout.receive = AsyncMock(
                    return_value=b"2.0.0 (Codex)"
                )
                mock_version_process.terminate = MagicMock()
                mock_version_process.wait = AsyncMock()

                # Mock main process
                mock_process = MagicMock()
                mock_process.stdout = MagicMock()
                mock_stdin = MagicMock()
                mock_stdin.aclose = AsyncMock()  # Add async aclose method
                mock_process.stdin = mock_stdin
                mock_process.returncode = None

                # Return version process first, then main process
                mock_open_process.side_effect = [mock_version_process, mock_process]

                transport = SubprocessCLITransport(
                    prompt="test",
                    options=options,
                )

                await transport.connect()

                # Verify open_process was called twice (version check + main process)
                assert mock_open_process.call_count == 2

                # Check the second call (main process) for user
                second_call_kwargs = mock_open_process.call_args_list[1].kwargs
                assert "user" in second_call_kwargs
                user_passed = second_call_kwargs["user"]

                # Check that user was passed
                assert user_passed == "testuser"

        anyio.run(_test)

    def test_build_command_with_sandbox_only(self):
        """Test building CLI command with sandbox settings (no existing settings)."""
        import json

        from agent_sdk import SandboxSettings

        sandbox: SandboxSettings = {
            "enabled": True,
            "autoAllowBashIfSandboxed": True,
            "network": {
                "allowLocalBinding": True,
                "allowUnixSockets": ["/var/run/docker.sock"],
            },
        }

        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(sandbox=sandbox),
        )

        cmd = transport._build_command()

        # Should have --settings with sandbox merged in
        assert "--settings" in cmd
        settings_idx = cmd.index("--settings")
        settings_value = cmd[settings_idx + 1]

        # Parse and verify
        parsed = json.loads(settings_value)
        assert "sandbox" in parsed
        assert parsed["sandbox"]["enabled"] is True
        assert parsed["sandbox"]["autoAllowBashIfSandboxed"] is True
        assert parsed["sandbox"]["network"]["allowLocalBinding"] is True
        assert parsed["sandbox"]["network"]["allowUnixSockets"] == [
            "/var/run/docker.sock"
        ]

    def test_build_command_with_sandbox_and_settings_json(self):
        """Test building CLI command with sandbox merged into existing settings JSON."""
        import json

        from agent_sdk import SandboxSettings

        # Existing settings as JSON string
        existing_settings = (
            '{"permissions": {"allow": ["Bash(ls:*)"]}, "verbose": true}'
        )

        sandbox: SandboxSettings = {
            "enabled": True,
            "excludedCommands": ["git", "docker"],
        }

        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(settings=existing_settings, sandbox=sandbox),
        )

        cmd = transport._build_command()

        # Should have merged settings
        assert "--settings" in cmd
        settings_idx = cmd.index("--settings")
        settings_value = cmd[settings_idx + 1]

        parsed = json.loads(settings_value)

        # Original settings should be preserved
        assert parsed["permissions"] == {"allow": ["Bash(ls:*)"]}
        assert parsed["verbose"] is True

        # Sandbox should be merged in
        assert "sandbox" in parsed
        assert parsed["sandbox"]["enabled"] is True
        assert parsed["sandbox"]["excludedCommands"] == ["git", "docker"]

    def test_build_command_with_settings_file_and_no_sandbox(self):
        """Test that settings file path is passed through when no sandbox."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(settings="/path/to/settings.json"),
        )

        cmd = transport._build_command()

        # Should pass path directly, not parse it
        assert "--settings" in cmd
        settings_idx = cmd.index("--settings")
        assert cmd[settings_idx + 1] == "/path/to/settings.json"

    def test_build_command_sandbox_minimal(self):
        """Test sandbox with minimal configuration."""
        import json

        from agent_sdk import SandboxSettings

        sandbox: SandboxSettings = {"enabled": True}

        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(sandbox=sandbox),
        )

        cmd = transport._build_command()

        assert "--settings" in cmd
        settings_idx = cmd.index("--settings")
        settings_value = cmd[settings_idx + 1]

        parsed = json.loads(settings_value)
        assert parsed == {"sandbox": {"enabled": True}}

    def test_sandbox_network_config(self):
        """Test sandbox with full network configuration."""
        import json

        from agent_sdk import SandboxSettings

        sandbox: SandboxSettings = {
            "enabled": True,
            "network": {
                "allowUnixSockets": ["/tmp/ssh-agent.sock"],
                "allowAllUnixSockets": False,
                "allowLocalBinding": True,
                "httpProxyPort": 8080,
                "socksProxyPort": 8081,
            },
        }

        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(sandbox=sandbox),
        )

        cmd = transport._build_command()
        settings_idx = cmd.index("--settings")
        settings_value = cmd[settings_idx + 1]

        parsed = json.loads(settings_value)
        network = parsed["sandbox"]["network"]

        assert network["allowUnixSockets"] == ["/tmp/ssh-agent.sock"]
        assert network["allowAllUnixSockets"] is False
        assert network["allowLocalBinding"] is True
        assert network["httpProxyPort"] == 8080
        assert network["socksProxyPort"] == 8081

    def test_build_command_with_tools_array(self):
        """Test building CLI command with tools as array of tool names."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(tools=["Read", "Edit", "Bash"]),
        )

        cmd = transport._build_command()
        assert "--tools" in cmd
        tools_idx = cmd.index("--tools")
        assert cmd[tools_idx + 1] == "Read,Edit,Bash"

    def test_build_command_with_tools_empty_array(self):
        """Test building CLI command with tools as empty array (disables all tools)."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(tools=[]),
        )

        cmd = transport._build_command()
        assert "--tools" in cmd
        tools_idx = cmd.index("--tools")
        assert cmd[tools_idx + 1] == ""

    def test_build_command_with_tools_preset(self):
        """Test building CLI command with tools preset."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(tools={"type": "preset", "preset": "codex"}),
        )

        cmd = transport._build_command()
        assert "--tools" in cmd
        tools_idx = cmd.index("--tools")
        assert cmd[tools_idx + 1] == "default"

    def test_build_command_without_tools(self):
        """Test building CLI command without tools option (default None)."""
        transport = SubprocessCLITransport(
            prompt="test",
            options=make_options(),
        )

        cmd = transport._build_command()
        assert "--tools" not in cmd

    def test_concurrent_writes_are_serialized(self):
        """Test that concurrent write() calls are serialized by the lock.

        When parallel subagents invoke MCP tools, they trigger concurrent write()
        calls. Without the _write_lock, trio raises BusyResourceError.

        Uses a real subprocess with the same stream setup as production:
        process.stdin -> TextSendStream
        """

        async def _test():
            import sys
            from subprocess import PIPE

            from anyio.streams.text import TextSendStream

            # Create a real subprocess that consumes stdin (cross-platform)
            process = await anyio.open_process(
                [sys.executable, "-c", "import sys; sys.stdin.read()"],
                stdin=PIPE,
                stdout=PIPE,
                stderr=PIPE,
            )

            try:
                transport = SubprocessCLITransport(
                    prompt="test",
                    options=CodexAgentOptions(cli_path="/usr/bin/codex"),
                )

                # Same setup as production: TextSendStream wrapping process.stdin
                transport._ready = True
                transport._process = MagicMock(returncode=None)
                transport._stdin_stream = TextSendStream(process.stdin)

                # Spawn concurrent writes - the lock should serialize them
                num_writes = 10
                errors: list[Exception] = []

                async def do_write(i: int):
                    try:
                        await transport.write(f'{{"msg": {i}}}\n')
                    except Exception as e:
                        errors.append(e)

                async with anyio.create_task_group() as tg:
                    for i in range(num_writes):
                        tg.start_soon(do_write, i)

                # All writes should succeed - the lock serializes them
                assert len(errors) == 0, f"Got errors: {errors}"
            finally:
                process.terminate()
                await process.wait()

        anyio.run(_test, backend="trio")

    def test_concurrent_writes_fail_without_lock(self):
        """Verify that without the lock, concurrent writes cause BusyResourceError.

        Uses a real subprocess with the same stream setup as production.
        """

        async def _test():
            import sys
            from contextlib import asynccontextmanager
            from subprocess import PIPE

            from anyio.streams.text import TextSendStream

            # Create a real subprocess that consumes stdin (cross-platform)
            process = await anyio.open_process(
                [sys.executable, "-c", "import sys; sys.stdin.read()"],
                stdin=PIPE,
                stdout=PIPE,
                stderr=PIPE,
            )

            try:
                transport = SubprocessCLITransport(
                    prompt="test",
                    options=CodexAgentOptions(cli_path="/usr/bin/codex"),
                )

                # Same setup as production
                transport._ready = True
                transport._process = MagicMock(returncode=None)
                transport._stdin_stream = TextSendStream(process.stdin)

                # Replace lock with no-op to trigger the race condition
                class NoOpLock:
                    @asynccontextmanager
                    async def __call__(self):
                        yield

                    async def __aenter__(self):
                        return self

                    async def __aexit__(self, *args):
                        pass

                transport._write_lock = NoOpLock()

                # Spawn concurrent writes - should fail without lock
                num_writes = 10
                errors: list[Exception] = []

                async def do_write(i: int):
                    try:
                        await transport.write(f'{{"msg": {i}}}\n')
                    except Exception as e:
                        errors.append(e)

                async with anyio.create_task_group() as tg:
                    for i in range(num_writes):
                        tg.start_soon(do_write, i)

                # Should have gotten errors due to concurrent access
                assert len(errors) > 0, (
                    "Expected errors from concurrent access, but got none"
                )

                # Check that at least one error mentions the concurrent access
                error_strs = [str(e) for e in errors]
                assert any("another task" in s for s in error_strs), (
                    f"Expected 'another task' error, got: {error_strs}"
                )
            finally:
                process.terminate()
                await process.wait()

        anyio.run(_test, backend="trio")
