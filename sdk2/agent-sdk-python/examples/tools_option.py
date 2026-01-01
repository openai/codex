#!/usr/bin/env python3
"""Example demonstrating the tools option and verifying tools in system message."""

import anyio

from agent_sdk import (
    AssistantMessage,
    CodexAgentOptions,
    ResultMessage,
    SystemMessage,
    TextBlock,
    query,
)


async def tools_array_example():
    """Example with tools as array of specific tool names."""
    print("=== Tools Array Example ===")
    print("Setting tools=['Read', 'Glob', 'Grep']")
    print()

    options = CodexAgentOptions(
        tools=["Read", "Glob", "Grep"],
        max_turns=1,
    )

    async for message in query(
        prompt="What tools do you have available? Just list them briefly.",
        options=options,
    ):
        if isinstance(message, SystemMessage) and message.subtype == "init":
            tools = message.data.get("tools", [])
            print(f"Tools from system message: {tools}")
            print()
        elif isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Codex: {block.text}")
        elif isinstance(message, ResultMessage):
            if message.total_cost_usd:
                print(f"\nCost: ${message.total_cost_usd:.4f}")
    print()


async def tools_empty_array_example():
    """Example with tools as empty array (disables all built-in tools)."""
    print("=== Tools Empty Array Example ===")
    print("Setting tools=[] (disables all built-in tools)")
    print()

    options = CodexAgentOptions(
        tools=[],
        max_turns=1,
    )

    async for message in query(
        prompt="What tools do you have available? Just list them briefly.",
        options=options,
    ):
        if isinstance(message, SystemMessage) and message.subtype == "init":
            tools = message.data.get("tools", [])
            print(f"Tools from system message: {tools}")
            print()
        elif isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Codex: {block.text}")
        elif isinstance(message, ResultMessage):
            if message.total_cost_usd:
                print(f"\nCost: ${message.total_cost_usd:.4f}")
    print()


async def tools_preset_example():
    """Example with tools preset (all default Codex tools)."""
    print("=== Tools Preset Example ===")
    print("Setting tools={'type': 'preset', 'preset': 'codex'}")
    print()

    options = CodexAgentOptions(
        tools={"type": "preset", "preset": "codex"},
        max_turns=1,
    )

    async for message in query(
        prompt="What tools do you have available? Just list them briefly.",
        options=options,
    ):
        if isinstance(message, SystemMessage) and message.subtype == "init":
            tools = message.data.get("tools", [])
            print(f"Tools from system message ({len(tools)} tools): {tools[:5]}...")
            print()
        elif isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Codex: {block.text}")
        elif isinstance(message, ResultMessage):
            if message.total_cost_usd:
                print(f"\nCost: ${message.total_cost_usd:.4f}")
    print()


async def main():
    """Run all examples."""
    await tools_array_example()
    await tools_empty_array_example()
    await tools_preset_example()


if __name__ == "__main__":
    anyio.run(main)
