#!/usr/bin/env python3
"""Example: Calculator MCP Server.

This example demonstrates how to create an in-process MCP server with
calculator tools using the Codex SDK.

Unlike external MCP servers that require separate processes, this server
runs directly within your Python application, providing better performance
and simpler deployment.
"""

import asyncio
from typing import Any

from agent_sdk import (
    CodexAgentOptions,
    create_sdk_mcp_server,
    tool,
)

# Define calculator tools using the @tool decorator


@tool("add", "Add two numbers", {"a": float, "b": float})
async def add_numbers(args: dict[str, Any]) -> dict[str, Any]:
    """Add two numbers together."""
    result = args["a"] + args["b"]
    return {
        "content": [{"type": "text", "text": f"{args['a']} + {args['b']} = {result}"}]
    }


@tool("subtract", "Subtract one number from another", {"a": float, "b": float})
async def subtract_numbers(args: dict[str, Any]) -> dict[str, Any]:
    """Subtract b from a."""
    result = args["a"] - args["b"]
    return {
        "content": [{"type": "text", "text": f"{args['a']} - {args['b']} = {result}"}]
    }


@tool("multiply", "Multiply two numbers", {"a": float, "b": float})
async def multiply_numbers(args: dict[str, Any]) -> dict[str, Any]:
    """Multiply two numbers."""
    result = args["a"] * args["b"]
    return {
        "content": [{"type": "text", "text": f"{args['a']} × {args['b']} = {result}"}]
    }


@tool("divide", "Divide one number by another", {"a": float, "b": float})
async def divide_numbers(args: dict[str, Any]) -> dict[str, Any]:
    """Divide a by b."""
    if args["b"] == 0:
        return {
            "content": [
                {"type": "text", "text": "Error: Division by zero is not allowed"}
            ],
            "is_error": True,
        }

    result = args["a"] / args["b"]
    return {
        "content": [{"type": "text", "text": f"{args['a']} ÷ {args['b']} = {result}"}]
    }


@tool("sqrt", "Calculate square root", {"n": float})
async def square_root(args: dict[str, Any]) -> dict[str, Any]:
    """Calculate the square root of a number."""
    n = args["n"]
    if n < 0:
        return {
            "content": [
                {
                    "type": "text",
                    "text": f"Error: Cannot calculate square root of negative number {n}",
                }
            ],
            "is_error": True,
        }

    import math

    result = math.sqrt(n)
    return {"content": [{"type": "text", "text": f"√{n} = {result}"}]}


@tool("power", "Raise a number to a power", {"base": float, "exponent": float})
async def power(args: dict[str, Any]) -> dict[str, Any]:
    """Raise base to the exponent power."""
    result = args["base"] ** args["exponent"]
    return {
        "content": [
            {"type": "text", "text": f"{args['base']}^{args['exponent']} = {result}"}
        ]
    }


def display_message(msg):
    """Display message content in a clean format."""
    from agent_sdk import (
        AssistantMessage,
        ResultMessage,
        SystemMessage,
        TextBlock,
        ToolResultBlock,
        ToolUseBlock,
        UserMessage,
    )

    if isinstance(msg, UserMessage):
        for block in msg.content:
            if isinstance(block, TextBlock):
                print(f"User: {block.text}")
            elif isinstance(block, ToolResultBlock):
                print(
                    f"Tool Result: {block.content[:100] if block.content else 'None'}..."
                )
    elif isinstance(msg, AssistantMessage):
        for block in msg.content:
            if isinstance(block, TextBlock):
                print(f"Codex: {block.text}")
            elif isinstance(block, ToolUseBlock):
                print(f"Using tool: {block.name}")
                # Show tool inputs for calculator
                if block.input:
                    print(f"  Input: {block.input}")
    elif isinstance(msg, SystemMessage):
        # Ignore system messages
        pass
    elif isinstance(msg, ResultMessage):
        print("Result ended")
        if msg.total_cost_usd:
            print(f"Cost: ${msg.total_cost_usd:.6f}")


async def main():
    """Run example calculations using the SDK MCP server with streaming client."""
    from agent_sdk import CodexSDKClient

    # Create the calculator server with all tools
    calculator = create_sdk_mcp_server(
        name="calculator",
        version="2.0.0",
        tools=[
            add_numbers,
            subtract_numbers,
            multiply_numbers,
            divide_numbers,
            square_root,
            power,
        ],
    )

    # Configure Codex to use the calculator server with allowed tools
    # Pre-approve all calculator MCP tools so they can be used without permission prompts
    options = CodexAgentOptions(
        mcp_servers={"calc": calculator},
        allowed_tools=[
            "mcp__calc__add",
            "mcp__calc__subtract",
            "mcp__calc__multiply",
            "mcp__calc__divide",
            "mcp__calc__sqrt",
            "mcp__calc__power",
        ],
    )

    # Example prompts to demonstrate calculator usage
    prompts = [
        "List your tools",
        "Calculate 15 + 27",
        "What is 100 divided by 7?",
        "Calculate the square root of 144",
        "What is 2 raised to the power of 8?",
        "Calculate (12 + 8) * 3 - 10",  # Complex calculation
    ]

    for prompt in prompts:
        print(f"\n{'=' * 50}")
        print(f"Prompt: {prompt}")
        print(f"{'=' * 50}")

        async with CodexSDKClient(options=options) as client:
            await client.query(prompt)

            async for message in client.receive_response():
                display_message(message)


if __name__ == "__main__":
    asyncio.run(main())
