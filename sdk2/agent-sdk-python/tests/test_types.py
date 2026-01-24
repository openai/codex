"""Tests for Codex SDK type definitions."""

from agent_sdk import (
    AssistantMessage,
    CodexAgentOptions,
    ResultMessage,
)
from agent_sdk.types import (
    TextBlock,
    ThinkingBlock,
    ToolResultBlock,
    ToolUseBlock,
    UserMessage,
)


class TestMessageTypes:
    """Test message type creation and validation."""

    def test_user_message_creation(self):
        """Test creating a UserMessage."""
        msg = UserMessage(content="Hello, Codex!")
        assert msg.content == "Hello, Codex!"

    def test_assistant_message_with_text(self):
        """Test creating an AssistantMessage with text content."""
        text_block = TextBlock(text="Hello, human!")
        msg = AssistantMessage(content=[text_block], model="test-model")
        assert len(msg.content) == 1
        assert msg.content[0].text == "Hello, human!"

    def test_assistant_message_with_thinking(self):
        """Test creating an AssistantMessage with thinking content."""
        thinking_block = ThinkingBlock(thinking="I'm thinking...", signature="sig-123")
        msg = AssistantMessage(
            content=[thinking_block], model="test-model"
        )
        assert len(msg.content) == 1
        assert msg.content[0].thinking == "I'm thinking..."
        assert msg.content[0].signature == "sig-123"

    def test_tool_use_block(self):
        """Test creating a ToolUseBlock."""
        block = ToolUseBlock(
            id="tool-123", name="Read", input={"file_path": "/test.txt"}
        )
        assert block.id == "tool-123"
        assert block.name == "Read"
        assert block.input["file_path"] == "/test.txt"

    def test_tool_result_block(self):
        """Test creating a ToolResultBlock."""
        block = ToolResultBlock(
            tool_use_id="tool-123", content="File contents here", is_error=False
        )
        assert block.tool_use_id == "tool-123"
        assert block.content == "File contents here"
        assert block.is_error is False

    def test_result_message(self):
        """Test creating a ResultMessage."""
        msg = ResultMessage(
            subtype="success",
            duration_ms=1500,
            duration_api_ms=1200,
            is_error=False,
            num_turns=1,
            session_id="session-123",
            total_cost_usd=0.01,
        )
        assert msg.subtype == "success"
        assert msg.total_cost_usd == 0.01
        assert msg.session_id == "session-123"


class TestOptions:
    """Test Options configuration."""

    def test_default_options(self):
        """Test Options with default values."""
        options = CodexAgentOptions()
        assert options.allowed_tools == []
        assert options.system_prompt is None
        assert options.permission_mode is None
        assert options.continue_conversation is False
        assert options.disallowed_tools == []

    def test_codex_options_with_tools(self):
        """Test Options with built-in tools."""
        options = CodexAgentOptions(
            allowed_tools=["Read", "Write", "Edit"], disallowed_tools=["Bash"]
        )
        assert options.allowed_tools == ["Read", "Write", "Edit"]
        assert options.disallowed_tools == ["Bash"]

    def test_codex_options_with_permission_mode(self):
        """Test Options with permission mode."""
        options = CodexAgentOptions(permission_mode="bypassPermissions")
        assert options.permission_mode == "bypassPermissions"

        options_plan = CodexAgentOptions(permission_mode="plan")
        assert options_plan.permission_mode == "plan"

        options_default = CodexAgentOptions(permission_mode="default")
        assert options_default.permission_mode == "default"

        options_accept = CodexAgentOptions(permission_mode="acceptEdits")
        assert options_accept.permission_mode == "acceptEdits"

    def test_codex_options_with_system_prompt_string(self):
        """Test Options with system prompt as string."""
        options = CodexAgentOptions(
            system_prompt="You are a helpful assistant.",
        )
        assert options.system_prompt == "You are a helpful assistant."

    def test_codex_options_with_system_prompt_preset(self):
        """Test Options with system prompt preset."""
        options = CodexAgentOptions(
            system_prompt={"type": "preset", "preset": "codex"},
        )
        assert options.system_prompt == {"type": "preset", "preset": "codex"}

    def test_codex_options_with_system_prompt_preset_and_append(self):
        """Test Options with system prompt preset and append."""
        options = CodexAgentOptions(
            system_prompt={
                "type": "preset",
                "preset": "codex",
                "append": "Be concise.",
            },
        )
        assert options.system_prompt == {
            "type": "preset",
            "preset": "codex",
            "append": "Be concise.",
        }

    def test_codex_options_with_session_continuation(self):
        """Test Options with session continuation."""
        options = CodexAgentOptions(continue_conversation=True, resume="session-123")
        assert options.continue_conversation is True
        assert options.resume == "session-123"

    def test_codex_options_with_model_specification(self):
        """Test Options with model specification."""
        options = CodexAgentOptions(
            model="test-model", permission_prompt_tool_name="CustomTool"
        )
        assert options.model == "test-model"
        assert options.permission_prompt_tool_name == "CustomTool"
