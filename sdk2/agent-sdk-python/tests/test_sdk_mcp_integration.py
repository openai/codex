"""Integration tests for SDK MCP server support.

This test file verifies that SDK MCP servers work correctly through the full stack,
matching the TypeScript SDK test/sdk.test.ts pattern.
"""

import base64
from typing import Any

import pytest
from mcp.types import CallToolRequest, CallToolRequestParams

from agent_sdk import (
    CodexAgentOptions,
    create_sdk_mcp_server,
    tool,
)


@pytest.mark.asyncio
async def test_sdk_mcp_server_handlers():
    """Test that SDK MCP server handlers are properly registered."""
    # Track tool executions
    tool_executions: list[dict[str, Any]] = []

    # Create SDK MCP server with multiple tools
    @tool("greet_user", "Greets a user by name", {"name": str})
    async def greet_user(args: dict[str, Any]) -> dict[str, Any]:
        tool_executions.append({"name": "greet_user", "args": args})
        return {"content": [{"type": "text", "text": f"Hello, {args['name']}!"}]}

    @tool("add_numbers", "Adds two numbers", {"a": float, "b": float})
    async def add_numbers(args: dict[str, Any]) -> dict[str, Any]:
        tool_executions.append({"name": "add_numbers", "args": args})
        result = args["a"] + args["b"]
        return {"content": [{"type": "text", "text": f"The sum is {result}"}]}

    server_config = create_sdk_mcp_server(
        name="test-sdk-server", version="1.0.0", tools=[greet_user, add_numbers]
    )

    # Verify server configuration
    assert server_config["type"] == "sdk"
    assert server_config["name"] == "test-sdk-server"
    assert "instance" in server_config

    # Get the server instance
    server = server_config["instance"]

    # Import the request types to check handlers
    from mcp.types import CallToolRequest, ListToolsRequest

    # Verify handlers are registered
    assert ListToolsRequest in server.request_handlers
    assert CallToolRequest in server.request_handlers

    # Test list_tools handler - the decorator wraps our function
    list_handler = server.request_handlers[ListToolsRequest]
    request = ListToolsRequest(method="tools/list")
    response = await list_handler(request)
    # Response is ServerResult with nested ListToolsResult
    assert len(response.root.tools) == 2

    # Check tool definitions
    tool_names = [t.name for t in response.root.tools]
    assert "greet_user" in tool_names
    assert "add_numbers" in tool_names

    # Test call_tool handler
    call_handler = server.request_handlers[CallToolRequest]

    # Call greet_user - CallToolRequest wraps the call
    from mcp.types import CallToolRequestParams

    greet_request = CallToolRequest(
        method="tools/call",
        params=CallToolRequestParams(name="greet_user", arguments={"name": "Alice"}),
    )
    result = await call_handler(greet_request)
    # Response is ServerResult with nested CallToolResult
    assert result.root.content[0].text == "Hello, Alice!"
    assert len(tool_executions) == 1
    assert tool_executions[0]["name"] == "greet_user"
    assert tool_executions[0]["args"]["name"] == "Alice"

    # Call add_numbers
    add_request = CallToolRequest(
        method="tools/call",
        params=CallToolRequestParams(name="add_numbers", arguments={"a": 5, "b": 3}),
    )
    result = await call_handler(add_request)
    assert "8" in result.root.content[0].text
    assert len(tool_executions) == 2
    assert tool_executions[1]["name"] == "add_numbers"
    assert tool_executions[1]["args"]["a"] == 5
    assert tool_executions[1]["args"]["b"] == 3


@pytest.mark.asyncio
async def test_tool_creation():
    """Test that tools can be created with proper schemas."""

    @tool("echo", "Echo input", {"input": str})
    async def echo_tool(args: dict[str, Any]) -> dict[str, Any]:
        return {"output": args["input"]}

    # Verify tool was created
    assert echo_tool.name == "echo"
    assert echo_tool.description == "Echo input"
    assert echo_tool.input_schema == {"input": str}
    assert callable(echo_tool.handler)

    # Test the handler works
    result = await echo_tool.handler({"input": "test"})
    assert result == {"output": "test"}


@pytest.mark.asyncio
async def test_error_handling():
    """Test that tool errors are properly handled."""

    @tool("fail", "Always fails", {})
    async def fail_tool(args: dict[str, Any]) -> dict[str, Any]:
        raise ValueError("Expected error")

    # Verify the tool raises an error when called directly
    with pytest.raises(ValueError, match="Expected error"):
        await fail_tool.handler({})

    # Test error handling through the server
    server_config = create_sdk_mcp_server(name="error-test", tools=[fail_tool])

    server = server_config["instance"]
    from mcp.types import CallToolRequest

    call_handler = server.request_handlers[CallToolRequest]

    # The handler should return an error result, not raise
    from mcp.types import CallToolRequestParams

    fail_request = CallToolRequest(
        method="tools/call", params=CallToolRequestParams(name="fail", arguments={})
    )
    result = await call_handler(fail_request)
    # MCP SDK catches exceptions and returns error results
    assert result.root.isError
    assert "Expected error" in str(result.root.content[0].text)


@pytest.mark.asyncio
async def test_mixed_servers():
    """Test that SDK and external MCP servers can work together."""

    # Create an SDK server
    @tool("sdk_tool", "SDK tool", {})
    async def sdk_tool(args: dict[str, Any]) -> dict[str, Any]:
        return {"result": "from SDK"}

    sdk_server = create_sdk_mcp_server(name="sdk-server", tools=[sdk_tool])

    # Create configuration with both SDK and external servers
    external_server = {"type": "stdio", "command": "echo", "args": ["test"]}

    options = CodexAgentOptions(
        mcp_servers={"sdk": sdk_server, "external": external_server}
    )

    # Verify both server types are in the configuration
    assert "sdk" in options.mcp_servers
    assert "external" in options.mcp_servers
    assert options.mcp_servers["sdk"]["type"] == "sdk"
    assert options.mcp_servers["external"]["type"] == "stdio"


@pytest.mark.asyncio
async def test_server_creation():
    """Test that SDK MCP servers are created correctly."""
    server = create_sdk_mcp_server(name="test-server", version="2.0.0", tools=[])

    # Verify server configuration
    assert server["type"] == "sdk"
    assert server["name"] == "test-server"
    assert "instance" in server
    assert server["instance"] is not None

    # Verify the server instance has the right attributes
    instance = server["instance"]
    assert instance.name == "test-server"
    assert instance.version == "2.0.0"

    # With no tools, no handlers are registered if tools is empty
    from mcp.types import ListToolsRequest

    # When no tools are provided, the handlers are not registered
    assert ListToolsRequest not in instance.request_handlers


@pytest.mark.asyncio
async def test_image_content_support():
    """Test that tools can return image content with base64 data."""

    # Create sample base64 image data (a simple 1x1 pixel PNG)
    png_data = base64.b64encode(
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01"
        b"\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\tpHYs\x00\x00\x0b\x13"
        b"\x00\x00\x0b\x13\x01\x00\x9a\x9c\x18\x00\x00\x00\x0cIDATx\x9cc```"
        b"\x00\x00\x00\x04\x00\x01]U!\x1c\x00\x00\x00\x00IEND\xaeB`\x82"
    ).decode("utf-8")

    # Track tool executions
    tool_executions: list[dict[str, Any]] = []

    # Create a tool that returns both text and image content
    @tool(
        "generate_chart", "Generates a chart and returns it as an image", {"title": str}
    )
    async def generate_chart(args: dict[str, Any]) -> dict[str, Any]:
        tool_executions.append({"name": "generate_chart", "args": args})
        return {
            "content": [
                {"type": "text", "text": f"Generated chart: {args['title']}"},
                {
                    "type": "image",
                    "data": png_data,
                    "mimeType": "image/png",
                },
            ]
        }

    server_config = create_sdk_mcp_server(
        name="image-test-server", version="1.0.0", tools=[generate_chart]
    )

    # Get the server instance
    server = server_config["instance"]

    call_handler = server.request_handlers[CallToolRequest]

    # Call the chart generation tool
    chart_request = CallToolRequest(
        method="tools/call",
        params=CallToolRequestParams(
            name="generate_chart", arguments={"title": "Sales Report"}
        ),
    )
    result = await call_handler(chart_request)

    # Verify the result contains both text and image content
    assert len(result.root.content) == 2

    # Check text content
    text_content = result.root.content[0]
    assert text_content.type == "text"
    assert text_content.text == "Generated chart: Sales Report"

    # Check image content
    image_content = result.root.content[1]
    assert image_content.type == "image"
    assert image_content.data == png_data
    assert image_content.mimeType == "image/png"

    # Verify the tool was executed correctly
    assert len(tool_executions) == 1
    assert tool_executions[0]["name"] == "generate_chart"
    assert tool_executions[0]["args"]["title"] == "Sales Report"
