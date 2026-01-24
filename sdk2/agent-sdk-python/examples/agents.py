#!/usr/bin/env python3
"""Example of using custom agents with Codex SDK.

This example demonstrates how to define and use custom agents with specific
tools, prompts, and models.

Usage:
./examples/agents.py - Run the example
"""

import anyio

from agent_sdk import (
    AgentDefinition,
    AssistantMessage,
    CodexAgentOptions,
    ResultMessage,
    TextBlock,
    query,
)


async def code_reviewer_example():
    """Example using a custom code reviewer agent."""
    print("=== Code Reviewer Agent Example ===")

    options = CodexAgentOptions(
        agents={
            "code-reviewer": AgentDefinition(
                description="Reviews code for best practices and potential issues",
                prompt="You are a code reviewer. Analyze code for bugs, performance issues, "
                "security vulnerabilities, and adherence to best practices. "
                "Provide constructive feedback.",
                tools=["Read", "Grep"],
                model="sonnet",
            ),
        },
    )

    async for message in query(
        prompt="Use the code-reviewer agent to review the code in src/agent_sdk/types.py",
        options=options,
    ):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Codex: {block.text}")
        elif isinstance(message, ResultMessage) and message.total_cost_usd and message.total_cost_usd > 0:
            print(f"\nCost: ${message.total_cost_usd:.4f}")
    print()


async def documentation_writer_example():
    """Example using a documentation writer agent."""
    print("=== Documentation Writer Agent Example ===")

    options = CodexAgentOptions(
        agents={
            "doc-writer": AgentDefinition(
                description="Writes comprehensive documentation",
                prompt="You are a technical documentation expert. Write clear, comprehensive "
                "documentation with examples. Focus on clarity and completeness.",
                tools=["Read", "Write", "Edit"],
                model="sonnet",
            ),
        },
    )

    async for message in query(
        prompt="Use the doc-writer agent to explain what AgentDefinition is used for",
        options=options,
    ):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Codex: {block.text}")
        elif isinstance(message, ResultMessage) and message.total_cost_usd and message.total_cost_usd > 0:
            print(f"\nCost: ${message.total_cost_usd:.4f}")
    print()


async def multiple_agents_example():
    """Example with multiple custom agents."""
    print("=== Multiple Agents Example ===")

    options = CodexAgentOptions(
        agents={
            "analyzer": AgentDefinition(
                description="Analyzes code structure and patterns",
                prompt="You are a code analyzer. Examine code structure, patterns, and architecture.",
                tools=["Read", "Grep", "Glob"],
            ),
            "tester": AgentDefinition(
                description="Creates and runs tests",
                prompt="You are a testing expert. Write comprehensive tests and ensure code quality.",
                tools=["Read", "Write", "Bash"],
                model="sonnet",
            ),
        },
        setting_sources=["user", "project"],
    )

    async for message in query(
        prompt="Use the analyzer agent to find all Python files in the examples/ directory",
        options=options,
    ):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Codex: {block.text}")
        elif isinstance(message, ResultMessage) and message.total_cost_usd and message.total_cost_usd > 0:
            print(f"\nCost: ${message.total_cost_usd:.4f}")
    print()


async def main():
    """Run all agent examples."""
    await code_reviewer_example()
    await documentation_writer_example()
    await multiple_agents_example()


if __name__ == "__main__":
    anyio.run(main)
