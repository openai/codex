"""Tests for tool permission callbacks and hook callbacks."""

import json

import pytest

from agent_sdk import (
    CodexAgentOptions,
    HookContext,
    HookInput,
    HookJSONOutput,
    HookMatcher,
    PermissionResultAllow,
    PermissionResultDeny,
    ToolPermissionContext,
)
from agent_sdk._internal.query import Query
from agent_sdk._internal.transport import Transport


class MockTransport(Transport):
    """Mock transport for testing."""

    def __init__(self):
        self.written_messages = []
        self.messages_to_read = []
        self._connected = False

    async def connect(self) -> None:
        self._connected = True

    async def close(self) -> None:
        self._connected = False

    async def write(self, data: str) -> None:
        self.written_messages.append(data)

    async def end_input(self) -> None:
        pass

    def read_messages(self):
        async def _read():
            for msg in self.messages_to_read:
                yield msg

        return _read()

    def is_ready(self) -> bool:
        return self._connected


class TestToolPermissionCallbacks:
    """Test tool permission callback functionality."""

    @pytest.mark.asyncio
    async def test_permission_callback_allow(self):
        """Test callback that allows tool execution."""
        callback_invoked = False

        async def allow_callback(
            tool_name: str, input_data: dict, context: ToolPermissionContext
        ) -> PermissionResultAllow:
            nonlocal callback_invoked
            callback_invoked = True
            assert tool_name == "TestTool"
            assert input_data == {"param": "value"}
            return PermissionResultAllow()

        transport = MockTransport()
        query = Query(
            transport=transport,
            is_streaming_mode=True,
            can_use_tool=allow_callback,
            hooks=None,
        )

        # Simulate control request
        request = {
            "type": "control_request",
            "request_id": "test-1",
            "request": {
                "subtype": "can_use_tool",
                "tool_name": "TestTool",
                "input": {"param": "value"},
                "permission_suggestions": [],
            },
        }

        await query._handle_control_request(request)

        # Check callback was invoked
        assert callback_invoked

        # Check response was sent
        assert len(transport.written_messages) == 1
        response = transport.written_messages[0]
        assert '"behavior": "allow"' in response

    @pytest.mark.asyncio
    async def test_permission_callback_deny(self):
        """Test callback that denies tool execution."""

        async def deny_callback(
            tool_name: str, input_data: dict, context: ToolPermissionContext
        ) -> PermissionResultDeny:
            return PermissionResultDeny(message="Security policy violation")

        transport = MockTransport()
        query = Query(
            transport=transport,
            is_streaming_mode=True,
            can_use_tool=deny_callback,
            hooks=None,
        )

        request = {
            "type": "control_request",
            "request_id": "test-2",
            "request": {
                "subtype": "can_use_tool",
                "tool_name": "DangerousTool",
                "input": {"command": "rm -rf /"},
                "permission_suggestions": ["deny"],
            },
        }

        await query._handle_control_request(request)

        # Check response
        assert len(transport.written_messages) == 1
        response = transport.written_messages[0]
        assert '"behavior": "deny"' in response
        assert '"message": "Security policy violation"' in response

    @pytest.mark.asyncio
    async def test_permission_callback_input_modification(self):
        """Test callback that modifies tool input."""

        async def modify_callback(
            tool_name: str, input_data: dict, context: ToolPermissionContext
        ) -> PermissionResultAllow:
            # Modify the input to add safety flag
            modified_input = input_data.copy()
            modified_input["safe_mode"] = True
            return PermissionResultAllow(updated_input=modified_input)

        transport = MockTransport()
        query = Query(
            transport=transport,
            is_streaming_mode=True,
            can_use_tool=modify_callback,
            hooks=None,
        )

        request = {
            "type": "control_request",
            "request_id": "test-3",
            "request": {
                "subtype": "can_use_tool",
                "tool_name": "WriteTool",
                "input": {"file_path": "/etc/passwd"},
                "permission_suggestions": [],
            },
        }

        await query._handle_control_request(request)

        # Check response includes modified input
        assert len(transport.written_messages) == 1
        response = transport.written_messages[0]
        assert '"behavior": "allow"' in response
        assert '"safe_mode": true' in response

    @pytest.mark.asyncio
    async def test_callback_exception_handling(self):
        """Test that callback exceptions are properly handled."""

        async def error_callback(
            tool_name: str, input_data: dict, context: ToolPermissionContext
        ) -> PermissionResultAllow:
            raise ValueError("Callback error")

        transport = MockTransport()
        query = Query(
            transport=transport,
            is_streaming_mode=True,
            can_use_tool=error_callback,
            hooks=None,
        )

        request = {
            "type": "control_request",
            "request_id": "test-5",
            "request": {
                "subtype": "can_use_tool",
                "tool_name": "TestTool",
                "input": {},
                "permission_suggestions": [],
            },
        }

        await query._handle_control_request(request)

        # Check error response was sent
        assert len(transport.written_messages) == 1
        response = transport.written_messages[0]
        assert '"subtype": "error"' in response
        assert "Callback error" in response


class TestHookCallbacks:
    """Test hook callback functionality."""

    @pytest.mark.asyncio
    async def test_hook_execution(self):
        """Test that hooks are called at appropriate times."""
        hook_calls = []

        async def test_hook(
            input_data: HookInput, tool_use_id: str | None, context: HookContext
        ) -> dict:
            hook_calls.append({"input": input_data, "tool_use_id": tool_use_id})
            return {"processed": True}

        transport = MockTransport()

        # Create hooks configuration
        hooks = {
            "tool_use_start": [{"matcher": {"tool": "TestTool"}, "hooks": [test_hook]}]
        }

        query = Query(
            transport=transport, is_streaming_mode=True, can_use_tool=None, hooks=hooks
        )

        # Manually register the hook callback to avoid needing the full initialize flow
        callback_id = "test_hook_0"
        query.hook_callbacks[callback_id] = test_hook

        # Simulate hook callback request
        request = {
            "type": "control_request",
            "request_id": "test-hook-1",
            "request": {
                "subtype": "hook_callback",
                "callback_id": callback_id,
                "input": {"test": "data"},
                "tool_use_id": "tool-123",
            },
        }

        await query._handle_control_request(request)

        # Check hook was called
        assert len(hook_calls) == 1
        assert hook_calls[0]["input"] == {"test": "data"}
        assert hook_calls[0]["tool_use_id"] == "tool-123"

        # Check response
        assert len(transport.written_messages) > 0
        last_response = transport.written_messages[-1]
        assert '"processed": true' in last_response

    @pytest.mark.asyncio
    async def test_hook_output_fields(self):
        """Test that all SyncHookJSONOutput fields are properly handled."""

        # Test all SyncHookJSONOutput fields together
        async def comprehensive_hook(
            input_data: HookInput, tool_use_id: str | None, context: HookContext
        ) -> HookJSONOutput:
            return {
                # Control fields
                "continue_": True,
                "suppressOutput": False,
                "stopReason": "Test stop reason",
                # Decision fields
                "decision": "block",
                "systemMessage": "Test system message",
                "reason": "Test reason for blocking",
                # Hook-specific output with all PreToolUse fields
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": "Security policy violation",
                    "updatedInput": {"modified": "input"},
                },
            }

        transport = MockTransport()
        hooks = {
            "PreToolUse": [
                {"matcher": {"tool": "TestTool"}, "hooks": [comprehensive_hook]}
            ]
        }

        query = Query(
            transport=transport, is_streaming_mode=True, can_use_tool=None, hooks=hooks
        )

        callback_id = "test_comprehensive_hook"
        query.hook_callbacks[callback_id] = comprehensive_hook

        request = {
            "type": "control_request",
            "request_id": "test-comprehensive",
            "request": {
                "subtype": "hook_callback",
                "callback_id": callback_id,
                "input": {"test": "data"},
                "tool_use_id": "tool-456",
            },
        }

        await query._handle_control_request(request)

        # Check response contains all the fields
        assert len(transport.written_messages) > 0
        last_response = transport.written_messages[-1]

        # Parse the JSON response
        response_data = json.loads(last_response)
        # The hook result is nested at response.response
        result = response_data["response"]["response"]

        # Verify control fields are present and converted to CLI format
        assert result.get("continue") is True, (
            "continue_ should be converted to continue"
        )
        assert "continue_" not in result, "continue_ should not appear in CLI output"
        assert result.get("suppressOutput") is False
        assert result.get("stopReason") == "Test stop reason"

        # Verify decision fields are present
        assert result.get("decision") == "block"
        assert result.get("reason") == "Test reason for blocking"
        assert result.get("systemMessage") == "Test system message"

        # Verify hook-specific output is present
        hook_output = result.get("hookSpecificOutput", {})
        assert hook_output.get("hookEventName") == "PreToolUse"
        assert hook_output.get("permissionDecision") == "deny"
        assert (
            hook_output.get("permissionDecisionReason") == "Security policy violation"
        )
        assert "updatedInput" in hook_output

    @pytest.mark.asyncio
    async def test_async_hook_output(self):
        """Test AsyncHookJSONOutput type with proper async fields."""

        async def async_hook(
            input_data: HookInput, tool_use_id: str | None, context: HookContext
        ) -> HookJSONOutput:
            # Test that async hooks properly use async_ and asyncTimeout fields
            return {
                "async_": True,
                "asyncTimeout": 5000,
            }

        transport = MockTransport()
        hooks = {"PreToolUse": [{"matcher": None, "hooks": [async_hook]}]}

        query = Query(
            transport=transport, is_streaming_mode=True, can_use_tool=None, hooks=hooks
        )

        callback_id = "test_async_hook"
        query.hook_callbacks[callback_id] = async_hook

        request = {
            "type": "control_request",
            "request_id": "test-async",
            "request": {
                "subtype": "hook_callback",
                "callback_id": callback_id,
                "input": {"test": "async_data"},
                "tool_use_id": None,
            },
        }

        await query._handle_control_request(request)

        # Check response contains async fields
        assert len(transport.written_messages) > 0
        last_response = transport.written_messages[-1]

        # Parse the JSON response
        response_data = json.loads(last_response)
        # The hook result is nested at response.response
        result = response_data["response"]["response"]

        # The SDK should convert async_ to "async" for CLI compatibility
        assert result.get("async") is True, "async_ should be converted to async"
        assert "async_" not in result, "async_ should not appear in CLI output"
        assert result.get("asyncTimeout") == 5000

    @pytest.mark.asyncio
    async def test_field_name_conversion(self):
        """Test that Python-safe field names (async_, continue_) are converted to CLI format (async, continue)."""

        async def conversion_test_hook(
            input_data: HookInput, tool_use_id: str | None, context: HookContext
        ) -> HookJSONOutput:
            # Return both async_ and continue_ to test conversion
            return {
                "async_": True,
                "asyncTimeout": 10000,
                "continue_": False,
                "stopReason": "Testing field conversion",
                "systemMessage": "Fields should be converted",
            }

        transport = MockTransport()
        hooks = {"PreToolUse": [{"matcher": None, "hooks": [conversion_test_hook]}]}

        query = Query(
            transport=transport, is_streaming_mode=True, can_use_tool=None, hooks=hooks
        )

        callback_id = "test_conversion"
        query.hook_callbacks[callback_id] = conversion_test_hook

        request = {
            "type": "control_request",
            "request_id": "test-conversion",
            "request": {
                "subtype": "hook_callback",
                "callback_id": callback_id,
                "input": {"test": "data"},
                "tool_use_id": None,
            },
        }

        await query._handle_control_request(request)

        # Check response has converted field names
        assert len(transport.written_messages) > 0
        last_response = transport.written_messages[-1]

        response_data = json.loads(last_response)
        result = response_data["response"]["response"]

        # Verify async_ was converted to async
        assert result.get("async") is True, "async_ should be converted to async"
        assert "async_" not in result, "async_ should not appear in output"

        # Verify continue_ was converted to continue
        assert result.get("continue") is False, (
            "continue_ should be converted to continue"
        )
        assert "continue_" not in result, "continue_ should not appear in output"

        # Verify other fields are unchanged
        assert result.get("asyncTimeout") == 10000
        assert result.get("stopReason") == "Testing field conversion"
        assert result.get("systemMessage") == "Fields should be converted"


class TestCodexAgentOptionsIntegration:
    """Test that callbacks work through CodexAgentOptions."""

    def test_options_with_callbacks(self):
        """Test creating options with callbacks."""

        async def my_callback(
            tool_name: str, input_data: dict, context: ToolPermissionContext
        ) -> PermissionResultAllow:
            return PermissionResultAllow()

        async def my_hook(
            input_data: HookInput, tool_use_id: str | None, context: HookContext
        ) -> dict:
            return {}

        options = CodexAgentOptions(
            can_use_tool=my_callback,
            hooks={
                "tool_use_start": [
                    HookMatcher(matcher={"tool": "Bash"}, hooks=[my_hook])
                ]
            },
        )

        assert options.can_use_tool == my_callback
        assert "tool_use_start" in options.hooks
        assert len(options.hooks["tool_use_start"]) == 1
        assert options.hooks["tool_use_start"][0].hooks[0] == my_hook
