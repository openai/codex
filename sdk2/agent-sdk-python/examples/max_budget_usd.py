#!/usr/bin/env python3
"""Example demonstrating max_budget_usd option for cost control."""

import anyio

from agent_sdk import (
    AssistantMessage,
    CodexAgentOptions,
    ResultMessage,
    TextBlock,
    query,
)


async def without_budget():
    """Example without budget limit."""
    print("=== Without Budget Limit ===")

    async for message in query(prompt="What is 2 + 2?"):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Codex: {block.text}")
        elif isinstance(message, ResultMessage):
            if message.total_cost_usd:
                print(f"Total cost: ${message.total_cost_usd:.4f}")
            print(f"Status: {message.subtype}")
    print()


async def with_reasonable_budget():
    """Example with budget that won't be exceeded."""
    print("=== With Reasonable Budget ($0.10) ===")

    options = CodexAgentOptions(
        max_budget_usd=0.10,  # 10 cents - plenty for a simple query
    )

    async for message in query(prompt="What is 2 + 2?", options=options):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Codex: {block.text}")
        elif isinstance(message, ResultMessage):
            if message.total_cost_usd:
                print(f"Total cost: ${message.total_cost_usd:.4f}")
            print(f"Status: {message.subtype}")
    print()


async def with_tight_budget():
    """Example with very tight budget that will likely be exceeded."""
    print("=== With Tight Budget ($0.0001) ===")

    options = CodexAgentOptions(
        max_budget_usd=0.0001,  # Very small budget - will be exceeded quickly
    )

    async for message in query(
        prompt="Read the README.md file and summarize it", options=options
    ):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Codex: {block.text}")
        elif isinstance(message, ResultMessage):
            if message.total_cost_usd:
                print(f"Total cost: ${message.total_cost_usd:.4f}")
            print(f"Status: {message.subtype}")

            # Check if budget was exceeded
            if message.subtype == "error_max_budget_usd":
                print("⚠️  Budget limit exceeded!")
                print(
                    "Note: The cost may exceed the budget by up to one API call's worth"
                )
    print()


async def main():
    """Run all examples."""
    print("This example demonstrates using max_budget_usd to control API costs.\n")

    await without_budget()
    await with_reasonable_budget()
    await with_tight_budget()

    print(
        "\nNote: Budget checking happens after each API call completes,\n"
        "so the final cost may slightly exceed the specified budget.\n"
    )


if __name__ == "__main__":
    anyio.run(main)
