"""Tests for message parser error handling."""

import pytest

from agent_sdk._errors import MessageParseError
from agent_sdk._internal.message_parser import parse_message
from agent_sdk.types import (
    AssistantMessage,
    ResultMessage,
    SystemMessage,
    TextBlock,
    ThinkingBlock,
    ToolResultBlock,
    ToolUseBlock,
    UserMessage,
)


class TestMessageParser:
    """Test message parsing with the new exception behavior."""

    def test_parse_valid_user_message(self):
        """Test parsing a valid user message."""
        data = {
            "type": "user",
            "message": {"content": [{"type": "text", "text": "Hello"}]},
        }
        message = parse_message(data)
        assert isinstance(message, UserMessage)
        assert len(message.content) == 1
        assert isinstance(message.content[0], TextBlock)
        assert message.content[0].text == "Hello"

    def test_parse_user_message_with_uuid(self):
        """Test parsing a user message with uuid field (issue #414).

        The uuid field is needed for file checkpointing with rewind_files().
        """
        data = {
            "type": "user",
            "uuid": "msg-abc123-def456",
            "message": {"content": [{"type": "text", "text": "Hello"}]},
        }
        message = parse_message(data)
        assert isinstance(message, UserMessage)
        assert message.uuid == "msg-abc123-def456"
        assert len(message.content) == 1

    def test_parse_user_message_with_tool_use(self):
        """Test parsing a user message with tool_use block."""
        data = {
            "type": "user",
            "message": {
                "content": [
                    {"type": "text", "text": "Let me read this file"},
                    {
                        "type": "tool_use",
                        "id": "tool_456",
                        "name": "Read",
                        "input": {"file_path": "/example.txt"},
                    },
                ]
            },
        }
        message = parse_message(data)
        assert isinstance(message, UserMessage)
        assert len(message.content) == 2
        assert isinstance(message.content[0], TextBlock)
        assert isinstance(message.content[1], ToolUseBlock)
        assert message.content[1].id == "tool_456"
        assert message.content[1].name == "Read"
        assert message.content[1].input == {"file_path": "/example.txt"}

    def test_parse_user_message_with_tool_result(self):
        """Test parsing a user message with tool_result block."""
        data = {
            "type": "user",
            "message": {
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "tool_789",
                        "content": "File contents here",
                    }
                ]
            },
        }
        message = parse_message(data)
        assert isinstance(message, UserMessage)
        assert len(message.content) == 1
        assert isinstance(message.content[0], ToolResultBlock)
        assert message.content[0].tool_use_id == "tool_789"
        assert message.content[0].content == "File contents here"

    def test_parse_user_message_with_tool_result_error(self):
        """Test parsing a user message with error tool_result block."""
        data = {
            "type": "user",
            "message": {
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "tool_error",
                        "content": "File not found",
                        "is_error": True,
                    }
                ]
            },
        }
        message = parse_message(data)
        assert isinstance(message, UserMessage)
        assert len(message.content) == 1
        assert isinstance(message.content[0], ToolResultBlock)
        assert message.content[0].tool_use_id == "tool_error"
        assert message.content[0].content == "File not found"
        assert message.content[0].is_error is True

    def test_parse_user_message_with_mixed_content(self):
        """Test parsing a user message with mixed content blocks."""
        data = {
            "type": "user",
            "message": {
                "content": [
                    {"type": "text", "text": "Here's what I found:"},
                    {
                        "type": "tool_use",
                        "id": "use_1",
                        "name": "Search",
                        "input": {"query": "test"},
                    },
                    {
                        "type": "tool_result",
                        "tool_use_id": "use_1",
                        "content": "Search results",
                    },
                    {"type": "text", "text": "What do you think?"},
                ]
            },
        }
        message = parse_message(data)
        assert isinstance(message, UserMessage)
        assert len(message.content) == 4
        assert isinstance(message.content[0], TextBlock)
        assert isinstance(message.content[1], ToolUseBlock)
        assert isinstance(message.content[2], ToolResultBlock)
        assert isinstance(message.content[3], TextBlock)

    def test_parse_user_message_inside_subagent(self):
        """Test parsing a valid user message."""
        data = {
            "type": "user",
            "message": {"content": [{"type": "text", "text": "Hello"}]},
            "parent_tool_use_id": "toolu_01Xrwd5Y13sEHtzScxR77So8",
        }
        message = parse_message(data)
        assert isinstance(message, UserMessage)
        assert message.parent_tool_use_id == "toolu_01Xrwd5Y13sEHtzScxR77So8"

    def test_parse_valid_assistant_message(self):
        """Test parsing a valid assistant message."""
        data = {
            "type": "assistant",
            "message": {
                "content": [
                    {"type": "text", "text": "Hello"},
                    {
                        "type": "tool_use",
                        "id": "tool_123",
                        "name": "Read",
                        "input": {"file_path": "/test.txt"},
                    },
                ],
                "model": "test-model",
            },
        }
        message = parse_message(data)
        assert isinstance(message, AssistantMessage)
        assert len(message.content) == 2
        assert isinstance(message.content[0], TextBlock)
        assert isinstance(message.content[1], ToolUseBlock)

    def test_parse_assistant_message_with_thinking(self):
        """Test parsing an assistant message with thinking block."""
        data = {
            "type": "assistant",
            "message": {
                "content": [
                    {
                        "type": "thinking",
                        "thinking": "I'm thinking about the answer...",
                        "signature": "sig-123",
                    },
                    {"type": "text", "text": "Here's my response"},
                ],
                "model": "test-model",
            },
        }
        message = parse_message(data)
        assert isinstance(message, AssistantMessage)
        assert len(message.content) == 2
        assert isinstance(message.content[0], ThinkingBlock)
        assert message.content[0].thinking == "I'm thinking about the answer..."
        assert message.content[0].signature == "sig-123"
        assert isinstance(message.content[1], TextBlock)
        assert message.content[1].text == "Here's my response"

    def test_parse_valid_system_message(self):
        """Test parsing a valid system message."""
        data = {"type": "system", "subtype": "start"}
        message = parse_message(data)
        assert isinstance(message, SystemMessage)
        assert message.subtype == "start"

    def test_parse_assistant_message_inside_subagent(self):
        """Test parsing a valid assistant message."""
        data = {
            "type": "assistant",
            "message": {
                "content": [
                    {"type": "text", "text": "Hello"},
                    {
                        "type": "tool_use",
                        "id": "tool_123",
                        "name": "Read",
                        "input": {"file_path": "/test.txt"},
                    },
                ],
                "model": "test-model",
            },
            "parent_tool_use_id": "toolu_01Xrwd5Y13sEHtzScxR77So8",
        }
        message = parse_message(data)
        assert isinstance(message, AssistantMessage)
        assert message.parent_tool_use_id == "toolu_01Xrwd5Y13sEHtzScxR77So8"

    def test_parse_valid_result_message(self):
        """Test parsing a valid result message."""
        data = {
            "type": "result",
            "subtype": "success",
            "duration_ms": 1000,
            "duration_api_ms": 500,
            "is_error": False,
            "num_turns": 2,
            "session_id": "session_123",
        }
        message = parse_message(data)
        assert isinstance(message, ResultMessage)
        assert message.subtype == "success"

    def test_parse_invalid_data_type(self):
        """Test that non-dict data raises MessageParseError."""
        with pytest.raises(MessageParseError) as exc_info:
            parse_message("not a dict")  # type: ignore
        assert "Invalid message data type" in str(exc_info.value)
        assert "expected dict, got str" in str(exc_info.value)

    def test_parse_missing_type_field(self):
        """Test that missing 'type' field raises MessageParseError."""
        with pytest.raises(MessageParseError) as exc_info:
            parse_message({"message": {"content": []}})
        assert "Message missing 'type' field" in str(exc_info.value)

    def test_parse_unknown_message_type(self):
        """Test that unknown message type raises MessageParseError."""
        with pytest.raises(MessageParseError) as exc_info:
            parse_message({"type": "unknown_type"})
        assert "Unknown message type: unknown_type" in str(exc_info.value)

    def test_parse_user_message_missing_fields(self):
        """Test that user message with missing fields raises MessageParseError."""
        with pytest.raises(MessageParseError) as exc_info:
            parse_message({"type": "user"})
        assert "Missing required field in user message" in str(exc_info.value)

    def test_parse_assistant_message_missing_fields(self):
        """Test that assistant message with missing fields raises MessageParseError."""
        with pytest.raises(MessageParseError) as exc_info:
            parse_message({"type": "assistant"})
        assert "Missing required field in assistant message" in str(exc_info.value)

    def test_parse_system_message_missing_fields(self):
        """Test that system message with missing fields raises MessageParseError."""
        with pytest.raises(MessageParseError) as exc_info:
            parse_message({"type": "system"})
        assert "Missing required field in system message" in str(exc_info.value)

    def test_parse_result_message_missing_fields(self):
        """Test that result message with missing fields raises MessageParseError."""
        with pytest.raises(MessageParseError) as exc_info:
            parse_message({"type": "result", "subtype": "success"})
        assert "Missing required field in result message" in str(exc_info.value)

    def test_message_parse_error_contains_data(self):
        """Test that MessageParseError contains the original data."""
        data = {"type": "unknown", "some": "data"}
        with pytest.raises(MessageParseError) as exc_info:
            parse_message(data)
        assert exc_info.value.data == data
