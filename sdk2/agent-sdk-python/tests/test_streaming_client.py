"""Tests for CodexSDKClient streaming functionality and query() with async iterables."""

import asyncio
import json
import sys
import tempfile
from pathlib import Path
from unittest.mock import AsyncMock, Mock, patch

import anyio
import pytest

from agent_sdk import (
    AssistantMessage,
    CodexAgentOptions,
    CodexSDKClient,
    CLIConnectionError,
    ResultMessage,
    TextBlock,
    UserMessage,
    query,
)
from agent_sdk._internal.transport.subprocess_cli import SubprocessCLITransport


def create_mock_transport(with_init_response=True):
    """Create a properly configured mock transport.

    Args:
        with_init_response: If True, automatically respond to initialization request
    """
    mock_transport = AsyncMock()
    mock_transport.connect = AsyncMock()
    mock_transport.close = AsyncMock()
    mock_transport.end_input = AsyncMock()
    mock_transport.write = AsyncMock()
    mock_transport.is_ready = Mock(return_value=True)

    # Track written messages to simulate control protocol responses
    written_messages = []

    async def mock_write(data):
        written_messages.append(data)

    mock_transport.write.side_effect = mock_write

    # Default read_messages to handle control protocol
    async def control_protocol_generator():
        # Wait for initialization request if needed
        if with_init_response:
            # Wait a bit for the write to happen
            await asyncio.sleep(0.01)

            # Check if initialization was requested
            for msg_str in written_messages:
                try:
                    msg = json.loads(msg_str.strip())
                    if (
                        msg.get("type") == "control_request"
                        and msg.get("request", {}).get("subtype") == "initialize"
                    ):
                        # Send initialization response
                        yield {
                            "type": "control_response",
                            "response": {
                                "request_id": msg.get("request_id"),
                                "subtype": "success",
                                "commands": [],
                                "output_style": "default",
                            },
                        }
                        break
                except (json.JSONDecodeError, KeyError, AttributeError):
                    pass

            # Keep checking for other control requests (like interrupt)
            last_check = len(written_messages)
            timeout_counter = 0
            while timeout_counter < 100:  # Avoid infinite loop
                await asyncio.sleep(0.01)
                timeout_counter += 1

                # Check for new messages
                for msg_str in written_messages[last_check:]:
                    try:
                        msg = json.loads(msg_str.strip())
                        if msg.get("type") == "control_request":
                            subtype = msg.get("request", {}).get("subtype")
                            if subtype == "interrupt":
                                # Send interrupt response
                                yield {
                                    "type": "control_response",
                                    "response": {
                                        "request_id": msg.get("request_id"),
                                        "subtype": "success",
                                    },
                                }
                                return  # End after interrupt
                    except (json.JSONDecodeError, KeyError, AttributeError):
                        pass
                last_check = len(written_messages)

        # Then end the stream
        return

    mock_transport.read_messages = control_protocol_generator
    return mock_transport


class TestCodexSDKClientStreaming:
    """Test CodexSDKClient streaming functionality."""

    def test_auto_connect_with_context_manager(self):
        """Test automatic connection when using context manager."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                async with CodexSDKClient() as client:
                    # Verify connect was called
                    mock_transport.connect.assert_called_once()
                    assert client._transport is mock_transport

                # Verify disconnect was called on exit
                mock_transport.close.assert_called_once()

        anyio.run(_test)

    def test_manual_connect_disconnect(self):
        """Test manual connect and disconnect."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                client = CodexSDKClient()
                await client.connect()

                # Verify connect was called
                mock_transport.connect.assert_called_once()
                assert client._transport is mock_transport

                await client.disconnect()
                # Verify disconnect was called
                mock_transport.close.assert_called_once()
                assert client._transport is None

        anyio.run(_test)

    def test_connect_with_string_prompt(self):
        """Test connecting with a string prompt."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                client = CodexSDKClient()
                await client.connect("Hello Codex")

                # Verify transport was created with string prompt
                call_kwargs = mock_transport_class.call_args.kwargs
                assert call_kwargs["prompt"] == "Hello Codex"

        anyio.run(_test)

    def test_connect_with_async_iterable(self):
        """Test connecting with an async iterable."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                async def message_stream():
                    yield {"type": "user", "message": {"role": "user", "content": "Hi"}}
                    yield {
                        "type": "user",
                        "message": {"role": "user", "content": "Bye"},
                    }

                client = CodexSDKClient()
                stream = message_stream()
                await client.connect(stream)

                # Verify transport was created with async iterable
                call_kwargs = mock_transport_class.call_args.kwargs
                # Should be the same async iterator
                assert call_kwargs["prompt"] is stream

        anyio.run(_test)

    def test_query(self):
        """Test sending a query."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                async with CodexSDKClient() as client:
                    await client.query("Test message")

                    # Verify write was called with correct format
                    # Should have at least 2 writes: init request and user message
                    assert mock_transport.write.call_count >= 2

                    # Find the user message in the write calls
                    user_msg_found = False
                    for call in mock_transport.write.call_args_list:
                        data = call[0][0]
                        try:
                            msg = json.loads(data.strip())
                            if msg.get("type") == "user":
                                assert msg["message"]["content"] == "Test message"
                                assert msg["session_id"] == "default"
                                user_msg_found = True
                                break
                        except (json.JSONDecodeError, KeyError, AttributeError):
                            pass
                    assert user_msg_found, "User message not found in write calls"

        anyio.run(_test)

    def test_send_message_with_session_id(self):
        """Test sending a message with custom session ID."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                async with CodexSDKClient() as client:
                    await client.query("Test", session_id="custom-session")

                    # Find the user message with custom session ID
                    session_found = False
                    for call in mock_transport.write.call_args_list:
                        data = call[0][0]
                        try:
                            msg = json.loads(data.strip())
                            if msg.get("type") == "user":
                                assert msg["session_id"] == "custom-session"
                                session_found = True
                                break
                        except (json.JSONDecodeError, KeyError, AttributeError):
                            pass
                    assert session_found, "User message with custom session not found"

        anyio.run(_test)

    def test_send_message_not_connected(self):
        """Test sending message when not connected raises error."""

        async def _test():
            client = CodexSDKClient()
            with pytest.raises(CLIConnectionError, match="Not connected"):
                await client.query("Test")

        anyio.run(_test)

    def test_receive_messages(self):
        """Test receiving messages."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                # Mock the message stream with control protocol support
                async def mock_receive():
                    # First handle initialization
                    await asyncio.sleep(0.01)
                    written = mock_transport.write.call_args_list
                    for call in written:
                        data = call[0][0]
                        try:
                            msg = json.loads(data.strip())
                            if (
                                msg.get("type") == "control_request"
                                and msg.get("request", {}).get("subtype")
                                == "initialize"
                            ):
                                yield {
                                    "type": "control_response",
                                    "response": {
                                        "request_id": msg.get("request_id"),
                                        "subtype": "success",
                                        "commands": [],
                                        "output_style": "default",
                                    },
                                }
                                break
                        except (json.JSONDecodeError, KeyError, AttributeError):
                            pass

                    # Then yield the actual messages
                    yield {
                        "type": "assistant",
                        "message": {
                            "role": "assistant",
                            "content": [{"type": "text", "text": "Hello!"}],
                            "model": "test-model",
                        },
                    }
                    yield {
                        "type": "user",
                        "message": {"role": "user", "content": "Hi there"},
                    }

                mock_transport.read_messages = mock_receive

                async with CodexSDKClient() as client:
                    messages = []
                    async for msg in client.receive_messages():
                        messages.append(msg)
                        if len(messages) == 2:
                            break

                    assert len(messages) == 2
                    assert isinstance(messages[0], AssistantMessage)
                    assert isinstance(messages[0].content[0], TextBlock)
                    assert messages[0].content[0].text == "Hello!"
                    assert isinstance(messages[1], UserMessage)
                    assert messages[1].content == "Hi there"

        anyio.run(_test)

    def test_receive_response(self):
        """Test receive_response stops at ResultMessage."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                # Mock the message stream with control protocol support
                async def mock_receive():
                    # First handle initialization
                    await asyncio.sleep(0.01)
                    written = mock_transport.write.call_args_list
                    for call in written:
                        data = call[0][0]
                        try:
                            msg = json.loads(data.strip())
                            if (
                                msg.get("type") == "control_request"
                                and msg.get("request", {}).get("subtype")
                                == "initialize"
                            ):
                                yield {
                                    "type": "control_response",
                                    "response": {
                                        "request_id": msg.get("request_id"),
                                        "subtype": "success",
                                        "commands": [],
                                        "output_style": "default",
                                    },
                                }
                                break
                        except (json.JSONDecodeError, KeyError, AttributeError):
                            pass

                    # Then yield the actual messages
                    yield {
                        "type": "assistant",
                        "message": {
                            "role": "assistant",
                            "content": [{"type": "text", "text": "Answer"}],
                            "model": "test-model",
                        },
                    }
                    yield {
                        "type": "result",
                        "subtype": "success",
                        "duration_ms": 1000,
                        "duration_api_ms": 800,
                        "is_error": False,
                        "num_turns": 1,
                        "session_id": "test",
                        "total_cost_usd": 0.001,
                    }
                    # This should not be yielded
                    yield {
                        "type": "assistant",
                        "message": {
                            "role": "assistant",
                            "content": [
                                {"type": "text", "text": "Should not see this"}
                            ],
                        },
                        "model": "test-model",
                    }

                mock_transport.read_messages = mock_receive

                async with CodexSDKClient() as client:
                    messages = []
                    async for msg in client.receive_response():
                        messages.append(msg)

                    # Should only get 2 messages (assistant + result)
                    assert len(messages) == 2
                    assert isinstance(messages[0], AssistantMessage)
                    assert isinstance(messages[1], ResultMessage)

        anyio.run(_test)

    def test_interrupt(self):
        """Test interrupt functionality."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                async with CodexSDKClient() as client:
                    # Interrupt is now handled via control protocol
                    await client.interrupt()
                    # Check that a control request was sent via write
                    write_calls = mock_transport.write.call_args_list
                    interrupt_found = False
                    for call in write_calls:
                        data = call[0][0]
                        try:
                            msg = json.loads(data.strip())
                            if (
                                msg.get("type") == "control_request"
                                and msg.get("request", {}).get("subtype") == "interrupt"
                            ):
                                interrupt_found = True
                                break
                        except (json.JSONDecodeError, KeyError, AttributeError):
                            pass
                    assert interrupt_found, "Interrupt control request not found"

        anyio.run(_test)

    def test_interrupt_not_connected(self):
        """Test interrupt when not connected raises error."""

        async def _test():
            client = CodexSDKClient()
            with pytest.raises(CLIConnectionError, match="Not connected"):
                await client.interrupt()

        anyio.run(_test)

    def test_client_with_options(self):
        """Test client initialization with options."""

        async def _test():
            options = CodexAgentOptions(
                cwd="/custom/path",
                allowed_tools=["Read", "Write"],
                system_prompt="Be helpful",
            )

            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                client = CodexSDKClient(options=options)
                await client.connect()

                # Verify options were passed to transport
                call_kwargs = mock_transport_class.call_args.kwargs
                assert call_kwargs["options"] is options

        anyio.run(_test)

    def test_concurrent_send_receive(self):
        """Test concurrent sending and receiving messages."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                # Mock receive to wait then yield messages with control protocol support
                async def mock_receive():
                    # First handle initialization
                    await asyncio.sleep(0.01)
                    written = mock_transport.write.call_args_list
                    for call in written:
                        if call:
                            data = call[0][0]
                            try:
                                msg = json.loads(data.strip())
                                if (
                                    msg.get("type") == "control_request"
                                    and msg.get("request", {}).get("subtype")
                                    == "initialize"
                                ):
                                    yield {
                                        "type": "control_response",
                                        "response": {
                                            "request_id": msg.get("request_id"),
                                            "subtype": "success",
                                            "commands": [],
                                            "output_style": "default",
                                        },
                                    }
                                    break
                            except (json.JSONDecodeError, KeyError, AttributeError):
                                pass

                    # Then yield the actual messages
                    await asyncio.sleep(0.1)
                    yield {
                        "type": "assistant",
                        "message": {
                            "role": "assistant",
                            "content": [{"type": "text", "text": "Response 1"}],
                            "model": "test-model",
                        },
                    }
                    await asyncio.sleep(0.1)
                    yield {
                        "type": "result",
                        "subtype": "success",
                        "duration_ms": 1000,
                        "duration_api_ms": 800,
                        "is_error": False,
                        "num_turns": 1,
                        "session_id": "test",
                        "total_cost_usd": 0.001,
                    }

                mock_transport.read_messages = mock_receive

                async with CodexSDKClient() as client:
                    # Helper to get next message
                    async def get_next_message():
                        return await client.receive_response().__anext__()

                    # Start receiving in background
                    receive_task = asyncio.create_task(get_next_message())

                    # Send message while receiving
                    await client.query("Question 1")

                    # Wait for first message
                    first_msg = await receive_task
                    assert isinstance(first_msg, AssistantMessage)

        anyio.run(_test)


class TestQueryWithAsyncIterable:
    """Test query() function with async iterable inputs."""

    def test_query_with_async_iterable(self):
        """Test query with async iterable of messages."""

        async def _test():
            async def message_stream():
                yield {"type": "user", "message": {"role": "user", "content": "First"}}
                yield {"type": "user", "message": {"role": "user", "content": "Second"}}

            # Create a simple test script that validates stdin and outputs a result
            with tempfile.NamedTemporaryFile(mode="w", suffix=".py", delete=False) as f:
                test_script = f.name
                f.write("""#!/usr/bin/env python3
import sys
import json

# Read stdin messages
stdin_messages = []
while True:
    line = sys.stdin.readline()
    if not line:
        break

    try:
        msg = json.loads(line.strip())
        # Handle control requests
        if msg.get("type") == "control_request":
            request_id = msg.get("request_id")
            request = msg.get("request", {})

            # Send control response for initialize
            if request.get("subtype") == "initialize":
                response = {
                    "type": "control_response",
                    "response": {
                        "subtype": "success",
                        "request_id": request_id,
                        "response": {
                            "commands": [],
                            "output_style": "default"
                        }
                    }
                }
                print(json.dumps(response))
                sys.stdout.flush()
        else:
            stdin_messages.append(line.strip())
    except:
        stdin_messages.append(line.strip())

# Verify we got 2 user messages
assert len(stdin_messages) == 2
assert '"First"' in stdin_messages[0]
assert '"Second"' in stdin_messages[1]

# Output a valid result
print('{"type": "result", "subtype": "success", "duration_ms": 100, "duration_api_ms": 50, "is_error": false, "num_turns": 1, "session_id": "test", "total_cost_usd": 0.001}')
""")

            # Make script executable (Unix-style systems)
            if sys.platform != "win32":
                Path(test_script).chmod(0o755)

            try:
                # Mock _find_cli to return the test script path directly
                with patch.object(
                    SubprocessCLITransport, "_find_cli", return_value=test_script
                ):
                    # Mock _build_command to properly execute Python script
                    original_build_command = SubprocessCLITransport._build_command

                    def mock_build_command(self):
                        # Get original command
                        cmd = original_build_command(self)
                        # On Windows, we need to use python interpreter to run the script
                        if sys.platform == "win32":
                            # Replace first element with python interpreter and script
                            cmd[0:1] = [sys.executable, test_script]
                        else:
                            # On Unix, just use the script directly
                            cmd[0] = test_script
                        return cmd

                    with patch.object(
                        SubprocessCLITransport, "_build_command", mock_build_command
                    ):
                        # Run query with async iterable
                        messages = []
                        async for msg in query(prompt=message_stream()):
                            messages.append(msg)

                        # Should get the result message
                        assert len(messages) == 1
                        assert isinstance(messages[0], ResultMessage)
                        assert messages[0].subtype == "success"
            finally:
                # Clean up
                Path(test_script).unlink()

        anyio.run(_test)


class TestCodexSDKClientEdgeCases:
    """Test edge cases and error scenarios."""

    def test_receive_messages_not_connected(self):
        """Test receiving messages when not connected."""

        async def _test():
            client = CodexSDKClient()
            with pytest.raises(CLIConnectionError, match="Not connected"):
                async for _ in client.receive_messages():
                    pass

        anyio.run(_test)

    def test_receive_response_not_connected(self):
        """Test receive_response when not connected."""

        async def _test():
            client = CodexSDKClient()
            with pytest.raises(CLIConnectionError, match="Not connected"):
                async for _ in client.receive_response():
                    pass

        anyio.run(_test)

    def test_double_connect(self):
        """Test connecting twice."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                # Create a new mock transport for each call
                mock_transport_class.side_effect = [
                    create_mock_transport(),
                    create_mock_transport(),
                ]

                client = CodexSDKClient()
                await client.connect()
                # Second connect should create new transport
                await client.connect()

                # Should have been called twice
                assert mock_transport_class.call_count == 2

        anyio.run(_test)

    def test_disconnect_without_connect(self):
        """Test disconnecting without connecting first."""

        async def _test():
            client = CodexSDKClient()
            # Should not raise error
            await client.disconnect()

        anyio.run(_test)

    def test_context_manager_with_exception(self):
        """Test context manager cleans up on exception."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                with pytest.raises(ValueError):
                    async with CodexSDKClient():
                        raise ValueError("Test error")

                # Disconnect should still be called
                mock_transport.close.assert_called_once()

        anyio.run(_test)

    def test_receive_response_list_comprehension(self):
        """Test collecting messages with list comprehension as shown in examples."""

        async def _test():
            with patch(
                "agent_sdk._internal.transport.subprocess_cli.SubprocessCLITransport"
            ) as mock_transport_class:
                mock_transport = create_mock_transport()
                mock_transport_class.return_value = mock_transport

                # Mock the message stream with control protocol support
                async def mock_receive():
                    # First handle initialization
                    await asyncio.sleep(0.01)
                    written = mock_transport.write.call_args_list
                    for call in written:
                        if call:
                            data = call[0][0]
                            try:
                                msg = json.loads(data.strip())
                                if (
                                    msg.get("type") == "control_request"
                                    and msg.get("request", {}).get("subtype")
                                    == "initialize"
                                ):
                                    yield {
                                        "type": "control_response",
                                        "response": {
                                            "request_id": msg.get("request_id"),
                                            "subtype": "success",
                                            "commands": [],
                                            "output_style": "default",
                                        },
                                    }
                                    break
                            except (json.JSONDecodeError, KeyError, AttributeError):
                                pass

                    # Then yield the actual messages
                    yield {
                        "type": "assistant",
                        "message": {
                            "role": "assistant",
                            "content": [{"type": "text", "text": "Hello"}],
                            "model": "test-model",
                        },
                    }
                    yield {
                        "type": "assistant",
                        "message": {
                            "role": "assistant",
                            "content": [{"type": "text", "text": "World"}],
                            "model": "test-model",
                        },
                    }
                    yield {
                        "type": "result",
                        "subtype": "success",
                        "duration_ms": 1000,
                        "duration_api_ms": 800,
                        "is_error": False,
                        "num_turns": 1,
                        "session_id": "test",
                        "total_cost_usd": 0.001,
                    }

                mock_transport.read_messages = mock_receive

                async with CodexSDKClient() as client:
                    # Test list comprehension pattern from docstring
                    messages = [msg async for msg in client.receive_response()]

                    assert len(messages) == 3
                    assert all(
                        isinstance(msg, AssistantMessage | ResultMessage)
                        for msg in messages
                    )
                    assert isinstance(messages[-1], ResultMessage)

        anyio.run(_test)
