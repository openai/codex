#!/usr/bin/env python3
"""Example of loading filesystem-based agents via setting_sources.

This example demonstrates how to load agents defined in .codex/agents/ files
using the setting_sources option. This is different from inline AgentDefinition
objects - these agents are loaded from markdown files on disk.

This example tests the scenario from issue #406 where filesystem-based agents
loaded via setting_sources=["project"] may silently fail in certain environments.

Usage:
./examples/filesystem_agents.py
"""

import asyncio
from pathlib import Path

from agent_sdk import (
    AssistantMessage,
    CodexAgentOptions,
    CodexSDKClient,
    ResultMessage,
    SystemMessage,
    TextBlock,
)


def extract_agents(msg: SystemMessage) -> list[str]:
    """Extract agent names from system message init data."""
    if msg.subtype == "init":
        agents = msg.data.get("agents", [])
        # Agents can be either strings or dicts with a 'name' field
        result = []
        for a in agents:
            if isinstance(a, str):
                result.append(a)
            elif isinstance(a, dict):
                result.append(a.get("name", ""))
        return result
    return []


async def main():
    """Test loading filesystem-based agents."""
    print("=== Filesystem Agents Example ===")
    print("Testing: setting_sources=['project'] with .codex/agents/test-agent.md")
    print()

    # Use the SDK repo directory which has .codex/agents/test-agent.md
    sdk_dir = Path(__file__).parent.parent

    options = CodexAgentOptions(
        setting_sources=["project"],
        cwd=sdk_dir,
    )

    message_types: list[str] = []
    agents_found: list[str] = []

    async with CodexSDKClient(options=options) as client:
        await client.query("Say hello in exactly 3 words")

        async for msg in client.receive_response():
            message_types.append(type(msg).__name__)

            if isinstance(msg, SystemMessage) and msg.subtype == "init":
                agents_found = extract_agents(msg)
                print(f"Init message received. Agents loaded: {agents_found}")

            elif isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Assistant: {block.text}")

            elif isinstance(msg, ResultMessage):
                print(
                    f"Result: subtype={msg.subtype}, cost=${msg.total_cost_usd or 0:.4f}"
                )

    print()
    print("=== Summary ===")
    print(f"Message types received: {message_types}")
    print(f"Total messages: {len(message_types)}")

    # Validate the results
    has_init = "SystemMessage" in message_types
    has_assistant = "AssistantMessage" in message_types
    has_result = "ResultMessage" in message_types
    has_test_agent = "test-agent" in agents_found

    print()
    if has_init and has_assistant and has_result:
        print("SUCCESS: Received full response (init, assistant, result)")
    else:
        print("FAILURE: Did not receive full response")
        print(f"  - Init: {has_init}")
        print(f"  - Assistant: {has_assistant}")
        print(f"  - Result: {has_result}")

    if has_test_agent:
        print("SUCCESS: test-agent was loaded from filesystem")
    else:
        print("WARNING: test-agent was NOT loaded (may not exist in .codex/agents/)")


if __name__ == "__main__":
    asyncio.run(main())
