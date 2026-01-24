#!/usr/bin/env python3
"""Quick start example for Codex SDK."""

import anyio

from agent_sdk import (
    AssistantMessage,
    CodexAgentOptions,
    ResultMessage,
    TextBlock,
    query,
)


async def basic_example():
    """Basic example - simple question."""
    print("=== Basic Example ===")

    async for message in query(prompt="What is 2 + 2?"):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Codex: {block.text}")
    print()


async def with_options_example():
    """Example with custom options."""
    print("=== With Options Example ===")

    options = CodexAgentOptions(
        system_prompt="You are a helpful assistant that explains things simply.",
        max_turns=1,
    )

    async for message in query(
        prompt="Explain what Python is in one sentence.", options=options
    ):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Codex: {block.text}")
    print()


async def with_tools_example():
    """Example using tools."""
    print("=== With Tools Example ===")

    options = CodexAgentOptions(
        allowed_tools=["Read", "Write"],
        system_prompt="You are a helpful file assistant.",
    )

    async for message in query(
        prompt="Create a file called hello.txt with 'Hello, World!' in it",
        options=options,
    ):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Codex: {block.text}")
        elif isinstance(message, ResultMessage) and message.total_cost_usd > 0:
            print(f"\nCost: ${message.total_cost_usd:.4f}")
    print()


async def main():
    """Run all examples."""
    await basic_example()
    await with_options_example()
    await with_tools_example()


if __name__ == "__main__":
    anyio.run(main)
